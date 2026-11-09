// Deterministic browser-adapter contract test; no network or external packages.
const fs = require('fs');
const vm = require('vm');
const assert = require('assert');
const path = require('path');
const source = fs.readFileSync(path.join(__dirname, '../apps/worldwright-server/assets/app.js'), 'utf8');
let page = 0;
async function fixture() {
  page += 1;
  const elements = {};
  function element(id) {
    return elements[id] ||= {value:'', disabled:false, textContent:'', childElementCount:0,
      handlers:{}, addEventListener(event, fn){this.handlers[event]=fn;},
      appendChild(){this.childElementCount++;}, removeChild(){this.childElementCount--;},
      replaceChildren(){this.childElementCount=0;}};
  }
  const sockets=[];
  class Socket {
    static OPEN=1;
    constructor(){this.readyState=1;this.handlers={};this.sent=[];sockets.push(this);}
    addEventListener(event,fn){this.handlers[event]=fn;}
    send(value){this.sent.push(JSON.parse(value));}
    close(){this.readyState=3;}
  }
  const context = {
    document:{querySelector: selector=>element(selector.slice(1)), createElement:()=>element('item-'+Math.random())},
    window:{setInterval:()=>1,clearInterval:()=>{}},
    location:{protocol:'https:',host:'acceptance.invalid'},
    WebSocket:Socket,crypto:{randomUUID:()=>`${String(page).padStart(8,'0')}-0000-4000-8000-000000000000`},
    fetch:async()=>({ok:true}),console,
  };
  vm.runInNewContext(source,context);
  await element('login-form').handlers.submit({preventDefault(){}});
  const socket=sockets[0];
  socket.handlers.open();
  socket.handlers.message({data:JSON.stringify({protocol_version:1,stream_epoch:'a'.repeat(32),sequence:1,message:{type:'welcome'}})});
  socket.handlers.message({data:JSON.stringify({protocol_version:1,stream_epoch:'a'.repeat(32),sequence:2,message:{type:'attached',character_name:'Alice'}})});
  function command(value){element('command').value=value;element('command-form').handlers.submit({preventDefault(){}});return socket.sent.at(-1);}
  return {socket,command};
}
(async()=>{
  const first=await fixture();
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
  const last=first.socket.sent.at(-1).request_id;
  const second=await fixture();
  assert.notEqual(second.command('rest').request_id,last);
  const count=first.socket.sent.length;
  first.command('rest unexpected-argument');
  assert.equal(first.socket.sent.length,count);
  first.command('__proto__');
  assert.equal(first.socket.sent.length,count);
  console.log('PASS browser fixture: four action mappings, seven status aliases, page-unique bounded request IDs, invalid-input rejection');
})().catch(error=>{console.error(error);process.exit(1);});
