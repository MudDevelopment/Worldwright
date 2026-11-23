'use strict';
const assert=require('node:assert/strict');
const base=require('./world.json'),{edition}=require('./generate.cjs');
function check(w){
  assert.equal(w.schema_version,1);assert.equal(w.namespace,'umbar');assert.equal(w.rooms.length,100);
  const m=new Map(w.rooms.map(r=>[r.id,r]));assert.equal(m.size,100);assert.equal(new Set(w.rooms.map(r=>r.title)).size,100);
  const sizes={},reverse=new Map([...m.keys()].map(k=>[k,[]]));
  for(const r of m.values()){
    assert.match(r.id,/^umbar:[a-z]+\.[a-z]+$/);assert.ok(r.title.trim()&&r.description.trim());
    const group=r.id.split('.')[0];sizes[group]=(sizes[group]||0)+1;
    const dest=Object.values(r.exits);assert.ok(dest.length);assert.equal(new Set(dest).size,dest.length);
    for(const [label,to] of Object.entries(r.exits)){assert.match(label,/^[a-z][a-z0-9_-]*$/);assert.notEqual(to,r.id);assert.ok(m.has(to));reverse.get(to).push(r.id);}
  }
  assert.deepEqual(Object.values(sizes),Array(10).fill(10));
  for(const adj of [id=>Object.values(m.get(id).exits),id=>reverse.get(id)]){const seen=new Set([w.entry_room]);for(const id of seen)for(const to of adj(id))seen.add(to);assert.equal(seen.size,100);}
  return m;
}
const room=(m,k)=>m.get('umbar:rooms.'+k);
function solve(m,marks){let r=room(m,'first');for(const [i,mark] of marks.entries()){assert.equal(new Set(Object.values(r.exits).filter((_,j)=>Object.keys(r.exits)[j]!==mark)).size,2);for(const [label,to] of Object.entries(r.exits))if(label!==mark)assert.ok(['umbar:rooms.muster','umbar:rooms.slate'].includes(to));r=m.get(r.exits[mark]);assert.equal(r,room(m,i?'roll':'second'));}}
solve(check(base),['west','east']);const variants=new Set();
for(let i=0;i<128;i++){const seed='shift-'+i,w=edition(seed);assert.deepEqual(w,edition(seed));const m=check(w);const clues=[...room(m,'slate').description.matchAll(/witness: the ([a-z ]+) at ([^()]+?) \(off the ring at ([^)]+)\)/g)];assert.equal(clues.length,2);const marks=clues.map(([,job,site,gate])=>{const target=w.rooms.find(r=>r.title===site),entry=w.rooms.find(r=>r.title===gate);assert.equal(target.id.split('.')[0],entry.id.split('.')[0]);assert.ok(target.description.includes('for the '+job+' reads:'));return target.description.match(/reads: (north|east|west)\.$/)[1];});assert.notEqual(clues[0][2],clues[1][2]);solve(m,marks);variants.add(clues.map(c=>c[2]).join('/')+marks.join('/'));}
assert.ok(variants.size>50);assert.throws(()=>edition('../bad'));console.log(`PASS: 100 distinct rooms, ten districts, all reachable and returnable; base puzzle and 128 deterministic editions solved (${variants.size} distinct assignments).`);
