// Deterministic DOM adapter checks; not real browser or backend acceptance.
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const assert = require('assert');
const source = fs.readFileSync(path.join(__dirname, '../apps/worldwright-studio/assets/app.js'), 'utf8');
const elements = new Map();
class Element {
  constructor() { this.value=''; this.children=[]; this.handlers={}; this.dataset={}; this.validity=''; }
  set id(value) { this._id=value; elements.set(value,this); }
  get id() { return this._id; }
  append(...values) { this.children.push(...values); }
  prepend(...values) { this.children.unshift(...values); }
  replaceChildren(...values) { this.children=values; }
  setAttribute() {}
  addEventListener(name, fn) { (this.handlers[name] ||= []).push(fn); }
  setCustomValidity(value) { this.validity=value; }
  focus() {}
  close() {}
  showModal() {}
}
function element(id) {
  if (!elements.has(id)) { const value=new Element(); value.id=id; }
  return elements.get(id);
}
const caps={schema_version:1,max_rooms:1024,max_exits_per_room:64,max_text_bytes:4096,
  max_definition_bytes:256,max_exit_label_bytes:64,max_input_bytes:1048576};
let reply=caps;
const context=vm.createContext({TextEncoder,console,
  document:{getElementById:element,querySelectorAll:()=>[],createElement:()=>new Element()},
  window:{addEventListener(){},setTimeout:fn=>fn(),setInterval:()=>1,clearInterval(){}},
  fetch:async()=>({ok:true,status:200,json:async()=>reply}),
});
vm.runInContext(source.replace('void initialize();',''),context);
(async()=>{
  await vm.runInContext('loadCapabilities()',context);
  const rooms=Array.from({length:1024},(_,index)=>({id:`test:rooms.r${index}`,title:`Room ${index}`,description:'Room',exits:{}}));
  context.rooms=rooms;
  vm.runInContext('state.snapshot={revision:1,snapshot_sha256:"a".repeat(64),draft:{namespace:"test",entry_room:rooms[0].id,rooms}}; state.selectedRoomId=rooms[0].id; renderRoomList(); renderRoomOptions();',context);
  assert.equal(element('room-list').children.length,1024,'last 24 valid rooms must not be hidden');
  assert.equal(element('entry-room').children.length,1024);
  element('new-room-id').value='test:rooms.extra'; element('new-room-title').value='Extra';
  element('add-room-form').handlers.submit[0]({preventDefault(){}});
  assert.equal(rooms.length,1024);
  assert.equal(element('add-room-error').textContent,'Room limit reached.');
  for (let index=1;index<=64;index++) rooms[0].exits[`exit-${index}`]=rooms[index].id;
  vm.runInContext('state.snapshot.draft.rooms=rooms.slice(0,65); renderExits()',context);
  assert.equal(element('exit-list').children.length,64);
  element('add-exit').handlers.click[0]();
  assert.equal(Object.keys(rooms[0].exits).length,64);
  element('room-title').value='🙂'.repeat(1024);
  vm.runInContext('checkByteLimit(element("room-title"), capabilities.max_text_bytes)',context);
  assert.equal(element('room-title').validity,'');
  element('room-title').value+='🙂';
  vm.runInContext('checkByteLimit(element("room-title"), capabilities.max_text_bytes)',context);
  assert(element('room-title').validity.includes('4096'));
  reply={...caps,max_rooms:0};
  await assert.rejects(vm.runInContext('loadCapabilities()',context), /Unsupported Studio capabilities/);
  console.log('PASS: 1024 rooms visible, 64-exit cap, UTF-8 byte boundaries, invalid capabilities rejected');
})().catch(error=>{console.error(error);process.exit(1);});
