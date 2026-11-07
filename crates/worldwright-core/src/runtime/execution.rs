use super::state::{ScopedView, State};
use super::{AddressedMessage, ExecutionResult, MAX_DELIVERIES, WorldError, WorldRuntime};
use crate::{
    command::{ActorContext, CommandEnvelope, CommandIntent},
    effect::{Audience, DomainEvent, Effect},
    time::{RandomSource, Tick},
};

impl WorldRuntime {
    /// Execute one command synchronously with exclusive world ownership.
    /// On Err, owned world state and its revision/time are unchanged; no events
    /// or messages are returned. Rule-owned state and caller RNG are NOT rolled
    /// back. Native rules must not perform ambient side effects.
    /// Operation IDs are correlation only; duplicate suppression is not provided.
    pub fn execute(
        &mut self,
        envelope: &CommandEnvelope,
        now: Tick,
        random: &mut dyn RandomSource,
    ) -> Result<ExecutionResult, WorldError> {
        if envelope.world != self.state.id {
            return Err(WorldError::WrongWorld);
        }
        self.state.validate_actor(&envelope.actor)?;
        if self.state.last_observed.is_some_and(|last| now < last) {
            return Err(WorldError::ClockRegression);
        }
        if let CommandIntent::Move { destination } = &envelope.intent {
            let from = self.state.current_room(envelope.actor.character)?;
            validate_edge(&self.state, from, *destination)?;
        }
        let revision = self.state.next_revision()?;
        let view = ScopedView {
            state: &self.state,
            actor: envelope.actor.character,
        };
        let plan = self.rules.plan(envelope, &view, now, random)?;
        // Exclusive &mut self prevents an attachment change during evaluation.
        // Keep the explicit check at the commit boundary for future refactoring.
        self.state.validate_actor(&envelope.actor)?;
        let mut staged = self.state.clone();
        let mut events = Vec::new();
        let mut messages = Vec::new();
        let mut moved = false;

        for effect in plan.into_effects() {
            match effect {
                Effect::MoveCharacter {
                    character,
                    destination,
                } => {
                    if character != envelope.actor.character
                        || moved
                        || !matches!(&envelope.intent, CommandIntent::Move { destination: requested } if *requested == destination)
                    {
                        return Err(WorldError::IntentViolation);
                    }
                    let from = staged.current_room(character)?;
                    validate_edge(&staged, from, destination)?;
                    staged
                        .characters
                        .get_mut(&character)
                        .ok_or(WorldError::UnknownCharacter)?
                        .room = destination;
                    events.push(DomainEvent::CharacterMoved {
                        character,
                        from,
                        to: destination,
                    });
                    moved = true;
                }
                Effect::EmitMessage { audience, text } => {
                    let recipients =
                        resolve_audience(&staged, &envelope.actor, &envelope.intent, audience)?;
                    if messages.len() + recipients.len() > MAX_DELIVERIES {
                        return Err(WorldError::DeliveryLimit);
                    }
                    for recipient in recipients {
                        messages.push(AddressedMessage {
                            recipient,
                            source: envelope.actor.character,
                            text: text.clone(),
                        });
                    }
                }
                Effect::AdjustCounter {
                    character,
                    key,
                    delta,
                } => {
                    if character != envelope.actor.character
                        || delta == 0
                        || !matches!(
                            envelope.intent,
                            CommandIntent::GameAction { .. } | CommandIntent::Move { .. }
                        )
                    {
                        return Err(WorldError::IntentViolation);
                    }
                    let before = staged
                        .counters
                        .get(&(character, key.clone()))
                        .copied()
                        .unwrap_or(0);
                    let after = if delta.is_negative() {
                        before.checked_sub(delta.unsigned_abs())
                    } else {
                        before.checked_add(delta as u64)
                    }
                    .ok_or(WorldError::CounterLimit)?;
                    if after > super::MAX_COUNTER_VALUE {
                        return Err(WorldError::CounterLimit);
                    }
                    staged.counters.insert((character, key.clone()), after);
                    events.push(DomainEvent::CounterAdjusted {
                        character,
                        key,
                        before,
                        after,
                    });
                }
                Effect::Schedule { at, action } => match action {
                    crate::effect::ScheduledAction::WakeCharacter(_) => {
                        return Err(WorldError::UnsupportedEffect);
                    }
                    crate::effect::ScheduledAction::AdjustCounter {
                        character,
                        key,
                        delta,
                    } if character == envelope.actor.character && delta != 0 && at > now => {
                        if !matches!(envelope.intent, CommandIntent::GameAction { .. }) {
                            return Err(WorldError::IntentViolation);
                        }
                        staged
                            .scheduler
                            .schedule(
                                at,
                                crate::effect::ScheduledAction::AdjustCounter {
                                    character,
                                    key: key.clone(),
                                    delta,
                                },
                            )
                            .map_err(|e| match e {
                                crate::scheduler::ScheduleError::Full => WorldError::ScheduleLimit,
                                crate::scheduler::ScheduleError::SequenceExhausted => {
                                    WorldError::CounterExhausted
                                }
                                _ => WorldError::ScheduleClock,
                            })?;
                        events.push(DomainEvent::CounterAdjustmentScheduled { character, key, at });
                    }
                    _ => return Err(WorldError::UnsupportedEffect),
                },
                Effect::TransferItem { .. } => {
                    return Err(WorldError::UnsupportedEffect);
                }
            }
        }
        if matches!(envelope.intent, CommandIntent::Move { .. }) && !moved {
            return Err(WorldError::MissingMovement);
        }
        staged.revision = revision;
        staged.last_execution = Some(now);
        staged.last_observed = Some(now);
        let result = ExecutionResult {
            operation: envelope.operation,
            world: staged.id,
            revision,
            events,
            messages,
        };
        self.state = staged;
        Ok(result)
    }
}

fn validate_edge(
    state: &State,
    from: crate::ids::RoomId,
    to: crate::ids::RoomId,
) -> Result<(), WorldError> {
    if !state.rooms.contains_key(&to) {
        return Err(WorldError::UnknownRoom);
    }
    if !state.can_traverse(from, to) {
        return Err(WorldError::Unreachable);
    }
    Ok(())
}

fn resolve_audience(
    state: &State,
    actor: &ActorContext,
    intent: &CommandIntent,
    audience: Audience,
) -> Result<Vec<ActorContext>, WorldError> {
    match audience {
        Audience::Character(target) => {
            if target != actor.character {
                return Err(WorldError::AudienceDenied);
            }
            Ok(vec![
                *state
                    .attachments
                    .get(&target)
                    .ok_or(WorldError::NotAttached)?,
            ])
        }
        Audience::Room(target) => {
            // Look descriptions are private to their requesting character.
            if matches!(intent, CommandIntent::Look)
                || state.current_room(actor.character)? != target
            {
                return Err(WorldError::AudienceDenied);
            }
            Ok(state
                .attachments
                .values()
                .filter(|recipient| {
                    state
                        .characters
                        .get(&recipient.character)
                        .is_some_and(|record| record.room == target)
                })
                .copied()
                .collect())
        }
    }
}
