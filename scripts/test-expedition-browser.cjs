// Built-in-only browser adapter contract tests; no network or fake DB proof.
const fs = require('fs');
const vm = require('vm');
const assert = require('assert');
const path = require('path');
const source = fs.readFileSync(path.join(__dirname, '../apps/worldwright-server/assets/app.js'), 'utf8');
let page = 0;
async function fixture({autoLogin=true, resumeStatus=401, fetchHook=null} = {}) {
  const pageId = ++page;
  const elements = {};
  const timers = new Set();
  let timerId = 0;
  function element(id) {
    return elements[id] ||= { value:'', disabled:false, textContent:'', childElementCount:0,
      handlers:{}, addEventListener(event, fn){this.handlers[event]=fn;},
      appendChild(){this.childElementCount++;}, removeChild(){this.childElementCount--;},
      replaceChildren(){this.childElementCount=0;} };
  }
  const sockets=[];
  const requests=[];
  class Socket {
    static OPEN=1;
    constructor(){this.readyState=1;this.handlers={};this.sent=[];sockets.push(this);}
    addEventListener(event,fn){this.handlers[event]=fn;}
    send(value){this.sent.push(JSON.parse(value));}
    close(){this.readyState=3;}
  }
  const context = {
    document:{querySelector: selector=>element(selector.slice(1)), createElement:()=>element('output-item')},
    window:{setInterval:()=>{timers.add(++timerId);return timerId;},clearInterval:id=>timers.delete(id)},
    location:{protocol:'https:',host:'acceptance.invalid'}, WebSocket:Socket,
    crypto:{randomUUID:()=>`${String(pageId).padStart(8,'0')}-0000-4000-8000-000000000000`},
    fetch:async(url, options)=>{
      requests.push({url, options});
      if (fetchHook) return fetchHook(options.method);
      return {ok: options.method !== 'GET' || resumeStatus === 204, status: options.method === 'GET' ? resumeStatus : 204};
    },console,
  };
  vm.runInNewContext(source,context);
  function message(socket, sequence, body) {
    socket.handlers.message({data:JSON.stringify({protocol_version:1,stream_epoch:'a'.repeat(32),sequence,message:body})});
  }
  async function login() {
    await element('login-form').handlers.submit({preventDefault(){}});
    const socket=sockets.at(-1);
    socket.handlers.open();
    message(socket,1,{type:'welcome'});
    message(socket,2,{type:'attached',character_name:'Alice'});
    return socket;
  }
  function command(value) {
    element('command').value=value;
    element('command-form').handlers.submit({preventDefault(){}});
    return sockets.at(-1).sent.at(-1);
  }
  await new Promise(resolve=>setImmediate(resolve));
  const socket=autoLogin ? await login() : sockets.at(-1);
  return {socket,sockets,command,login,message,timers,element,requests,
    logout:()=>element('logout').handlers.click()};
}
(async()=>{
  for (const resumeStatus of [401,503]) {
    const page=await fixture({autoLogin:false,resumeStatus});
    assert.equal(page.sockets.length,0,'failed cookie probe cannot connect');
    assert.equal(page.element('command-submit').disabled,true);
    assert.equal(page.requests[0].options.credentials,'same-origin');
    assert.equal(page.requests[0].options.cache,'no-store');
    assert(!page.requests[0].options.body,'resume sends no password or cookie body');
  }
  const resumed=await fixture({autoLogin:false,resumeStatus:204});
  assert.equal(resumed.sockets.length,1,'valid cookie reconnects without POST login');
  assert.equal(resumed.element('command-submit').disabled,true,'wait for authenticated attachment');
  resumed.socket.handlers.open();
  resumed.message(resumed.socket,1,{type:'welcome'});
  resumed.message(resumed.socket,2,{type:'attached',character_name:'Alice'});
  assert.equal(resumed.element('command-submit').disabled,false);
  assert.equal(resumed.requests.filter(request=>request.options.method==='POST').length,0);
  assert.equal(resumed.socket.sent.filter(message=>message.type==='command').length,0,'resume never resends old commands');
  let resolveProbe;
  const pending=await fixture({autoLogin:false,fetchHook:method=>method==='GET' ?
    new Promise(resolve=>{resolveProbe=resolve;}) : Promise.resolve({ok:true,status:204})});
  await pending.logout();
  resolveProbe({ok:true,status:204});
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal(pending.sockets.length,0,'late cookie probe cannot reconnect after logout');
  const first=await fixture();
  const firstAttach=first.socket.sent.find(message=>message.type==='attach').request_id;
  const firstCommand=first.command('forage');
  assert.equal(firstCommand.command.action,'expedition:actions.forage');
  for (const word of ['forage','encounter','strike','rest']) {
    const message=first.command(word);
    assert.equal(message.command.type,'game_action');
    assert.equal(message.command.action,`expedition:actions.${word}`);
    assert.equal(message.command.argument,null);
    assert.match(message.request_id,/^browser-[0-9a-f]{32}-\d+$/);
    assert(message.request_id.length<=128);
  }
  for (const word of ['status','supplies','threat','victories','rank','look','l']) {
    assert.equal(first.command(word).command.type,'look');
  }
  // Compare matching sequence numbers, not merely different commands/counts.
  for (const label of ['reload','new tab']) {
    const next=await fixture();
    const attach=next.socket.sent.find(message=>message.type==='attach').request_id;
    const command=next.command('forage');
    assert.notEqual(attach,firstAttach,`${label} attachment ID`);
    assert.notEqual(command.request_id,firstCommand.request_id,`${label} command ID`);
    assert.equal(command.request_id.split('-').at(-1),firstCommand.request_id.split('-').at(-1));
    assert.equal(next.socket.sent.filter(message=>message.type==='command').length,1,'no automatic replay on a fresh page');
  }
  const old=first.socket;
  const before=old.sent.filter(message=>message.request_id).map(message=>message.request_id);
  const current=await first.login();
  const attach=current.sent.find(message=>message.type==='attach').request_id;
  assert(!before.includes(attach),'reconnect must not reset page request counter');
  const look=first.command('look');
  assert(!before.includes(look.request_id));
  assert.equal(first.element('command-submit').disabled,false);
  old.handlers.close();
  assert.equal(first.element('command-submit').disabled,false,'stale close cannot disable replacement');
  assert.equal(first.timers.size,1,'old heartbeat removed');
  old.handlers.message({data:'not JSON'});
  assert.equal(current.readyState,1,'stale messages ignored');
  const count=current.sent.length;
  // Completion/replay/receipt-conflict messages must not manufacture retries.
  first.message(current,3,{type:'command_result',request_id:look.request_id,replayed:true});
  first.message(current,4,{type:'error',code:'receipt_conflict',message:'conflict',fatal:false});
  assert.equal(current.sent.length,count);
  assert.equal(current.readyState,1);
  first.command('rest unexpected-argument');
  first.command('__proto__');
  assert.equal(current.sent.length,count,'invalid commands not sent');
  const ids=first.sockets.flatMap(socket=>socket.sent.filter(message=>message.request_id).map(message=>message.request_id));
  assert.equal(new Set(ids).size,ids.length,'page lifecycle request IDs stay unique');
  await first.logout();
  assert.equal(first.timers.size,0);
  assert.equal(first.element('command-submit').disabled,true);
  first.command('rest');
  assert.equal(current.sent.length,count,'logged-out commands not sent');
  console.log('PASS: cookie resume/expiry/unavailability/logout race, actions/status, reload/new-tab IDs, reconnect, stale streams/timers, no automatic replay, logout');
})().catch(error=>{console.error(error);process.exit(1);});
