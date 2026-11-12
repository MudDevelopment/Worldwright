// Runs the actual Studio script with controlled DOM/HTTP timing, not a deployed Studio.
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const assert = require('assert/strict');
const source = fs.readFileSync(path.join(__dirname, '../apps/worldwright-studio/assets/app.js'), 'utf8');
const caps = {schema_version:1,max_rooms:1024,max_exits_per_room:64,max_text_bytes:4096,max_definition_bytes:256,max_exit_label_bytes:64,max_input_bytes:32*1024*1024};
const snapshot = (revision=1,title='Before') => ({revision,snapshot_sha256:(revision===1?'a':'b').repeat(64),draft:{schema_version:1,namespace:'test',entry_room:'test:rooms.start',rooms:[{id:'test:rooms.start',title,description:'Room',exits:{}}]}});
const artifact = {artifact_digest:'c'.repeat(64),payload_digest:'a'.repeat(64),draft_revision:1,created_at:1,release_id:'test'};
function harness() {
  const elements=new Map(), calls=[], timers=new Map(); let timer=0;
  class Element {
    constructor(){this.value='';this.children=[];this.handlers={};this.dataset={};this.disabled=false;this.hidden=false;this.textContent='';}
    set id(v){this._id=v;elements.set(v,this);} get id(){return this._id;}
    append(...v){this.children.push(...v);} prepend(...v){this.children.unshift(...v);} replaceChildren(...v){this.children=v;}
    addEventListener(n,f){(this.handlers[n] ||= []).push(f);} setAttribute(){} setCustomValidity(v){this.validity=v;}
    focus(){} close(){} showModal(){} scrollIntoView(){} reportValidity(){return true;} querySelector(){return new Element();}
  }
  function element(id){if(!elements.has(id)){const e=new Element();e.id=id;}return elements.get(id);}
  const context=vm.createContext({TextEncoder,TextDecoder,AbortController,console,
    setTimeout:(fn)=>{const id=++timer;timers.set(id,fn);return id;},clearTimeout:id=>timers.delete(id),
    document:{getElementById:element,querySelectorAll:()=>[],createElement:()=>new Element()},
    window:{addEventListener(){},setTimeout:fn=>{fn();return 0;},setInterval:()=>1,clearInterval(){},location:{assign(){}}},
    fetch:(url,options)=>new Promise((resolve,reject)=>{
      const call={url,options,resolve,reject};calls.push(call);
      options.signal.addEventListener('abort',()=>reject(new Error('aborted')),{once:true});
    }),
  });
  vm.runInContext(source.replace('void initialize();',''),context);
  context.fixture=snapshot(); context.caps=caps;
  vm.runInContext('capabilities=caps;MAX_ROOMS=caps.max_rooms;MAX_EXITS=caps.max_exits_per_room;state.snapshot=fixture;state.selectedRoomId=fixture.draft.entry_room;',context);
  element('package-game').value='expedition'; element('preview-seconds').value='30';
  function run(code){return vm.runInContext(code,context);}
  function event(id,type='click'){const e=element(id); return e.handlers[type][0]({preventDefault(){},currentTarget:e});}
  function reply(index,value,status=200){calls[index].resolve({ok:status>=200&&status<300,status,json:async()=>JSON.parse(JSON.stringify(value))});}
  function bindArtifact(){context.artifact=artifact;run('state.artifacts=[artifact];state.selectedArtifact=artifact.artifact_digest;state.artifactIdentity={...artifact,schema_version:1,game:"expedition"};');}
  return {context,run,event,element,calls,reply,timers,bindArtifact};
}
const tick=()=>new Promise(resolve=>setImmediate(resolve));
(async()=>{
 let count=0;
 async function test(name,fn){await fn();count++;console.log('PASS:',name);}
 await test('save preserves edits and selected new room while acknowledgement is pending',async()=>{
  const h=harness();h.run('markDirty()');const pending=h.event('world-form','submit');
  h.run('state.snapshot.draft.rooms.push({id:"test:rooms.new",title:"Local new",description:"",exits:{}});state.selectedRoomId="test:rooms.new";markDirty()');
  h.reply(0,snapshot(2,'Saved old'));await pending;
  assert.equal(h.run('state.snapshot.revision'),2);assert.equal(h.run('state.dirty'),true);
  assert.equal(h.run('state.snapshot.draft.rooms.length'),2);assert.equal(h.run('state.selectedRoomId'),'test:rooms.new');
  assert.equal(h.run('state.busy.size'),0);
 });
 await test('duplicate save suppressed and conflict preserves local content',async()=>{
  const h=harness();h.run('state.snapshot.draft.rooms[0].title="Local";markDirty()');
  const one=h.event('world-form','submit');await h.event('world-form','submit');assert.equal(h.calls.length,1);
  h.reply(0,{message:'conflict',current_revision:4},409);await one;
  assert.equal(h.run('state.snapshot.draft.rooms[0].title'),'Local');assert.equal(h.run('state.dirty'),true);assert.equal(h.element('conflict-panel').hidden,false);
 });
 await test('late reload never overwrites new edits',async()=>{
  const h=harness();const p=h.run('loadWorkspace({discard:true})');h.run('state.snapshot.draft.rooms[0].title="Keep";markDirty()');h.reply(0,snapshot(2));await p;
  assert.equal(h.run('state.snapshot.draft.rooms[0].title'),'Keep');assert.equal(h.run('state.snapshot.revision'),1);
 });
 await test('late validation cannot validate a newer draft',async()=>{
  const h=harness();const p=h.event('validate-draft');h.run('markDirty()');h.reply(0,{valid:true,snapshot_sha256:'a'.repeat(64),diagnostics:[]});await p;
  assert.equal(h.run('state.validation'),null);assert.equal(h.run('validationIsCurrent()'),false);
 });
 await test('late import keeps edits made during file/API read',async()=>{
  const h=harness();h.element('import-file').files=[{size:10,name:'fixture.json',text:async()=>'{"test":1}'}];
  const p=h.event('import-file','change');await tick();h.run('state.snapshot.draft.rooms[0].title="Keep";markDirty()');
  h.reply(0,{draft:snapshot(2,'Imported').draft,validation:{valid:true}});await p;
  assert.equal(h.run('state.snapshot.draft.rooms[0].title'),'Keep');assert.equal(h.run('state.busy.size'),0);
 });
 await test('preview polling does not overlap and old poll cannot replace stop result',async()=>{
  const h=harness();h.bindArtifact();h.run('state.preview={state:"running",artifact_digest:artifact.artifact_digest,draft_revision:1,snapshot_sha256:artifact.payload_digest,output:"old"}');
  const poll=h.run('loadPreview()');await h.run('loadPreview()');assert.equal(h.calls.length,1);
  const stop=h.event('preview-stop');h.reply(1,{state:'stopped',artifact_digest:artifact.artifact_digest,output:'new'});await stop;
  h.reply(0,{state:'running',artifact_digest:artifact.artifact_digest,output:'stale'});await poll;
  assert.equal(h.run('state.preview.state'),'stopped');assert.equal(h.run('state.preview.output'),'new');
 });
 await test('new preview input text survives an old send acknowledgement',async()=>{
  const h=harness();h.run('state.preview={state:"running"}');h.element('preview-input').value='look';const p=h.event('preview-input-form','submit');
  h.element('preview-input').value='forage';h.reply(0,null,204);await p;assert.equal(h.element('preview-input').value,'forage');
 });
 await test('late artifact game identity cannot replace current selection',async()=>{
  const h=harness();h.bindArtifact();const p=h.run('loadArtifactIdentity()');h.run('state.selectedArtifact="d".repeat(64)');
  h.reply(0,{...artifact,schema_version:1,game:'expedition'});await p;assert.equal(h.run('state.artifactIdentity'),null);
 });
 await test('artifact identity mismatch fails closed and preview requires full snapshot identity',async()=>{
  const h=harness();h.bindArtifact();const p=h.run('loadArtifactIdentity()');h.reply(0,{...artifact,schema_version:1,game:'expedition',payload_digest:'b'.repeat(64)});await p;
  assert.equal(h.run('state.artifactIdentity'),null);
  assert.equal(h.run('state.preview={state:"succeeded",artifact_digest:artifact.artifact_digest,draft_revision:2,snapshot_sha256:artifact.payload_digest};previewMatchesArtifact(artifact)'),false);
 });
 await test('401 submission remains one request and explains server-side renewal',async()=>{
  const h=harness();h.run('state.publicationAuth={configured:true,renewal:"token_file"}');const p=h.run('submitPublication({artifact_digest:"c".repeat(64)})');
  h.reply(0,{message:'expired'},401);await p;assert.equal(h.calls.length,1);assert.match(h.element('publication-note').textContent,/renew the private token file/);
  assert.equal(h.run('state.busy.size'),0);assert.equal(h.run('state.pendingPublication'),null);
 });
 await test('in-flight publication list cannot erase a newly submitted record',async()=>{
  const h=harness();const listing=h.run('loadPublications()');const submit=h.run('submitPublication({artifact_digest:"c".repeat(64)})');
  h.reply(1,{request_id:'1'.repeat(32),artifact_digest:'c'.repeat(64),status:'requested',target:'staging'});await submit;h.reply(0,[]);await listing;
  assert.equal(h.run('state.publications.length'),1);
 });
 await test('request timeout aborts and never retries mutations',async()=>{
  const h=harness();const p=h.run('api("/api/v1/package",{method:"POST"})');[...h.timers.values()].forEach(fn=>fn());
  await assert.rejects(p,/may have completed/);assert.equal(h.calls.length,1);assert.equal(h.calls[0].options.signal.aborted,true);assert.equal(h.calls[0].options.redirect,'error');
 });
 await test('streamed response body is capped and canceled before JSON parsing',async()=>{
  const h=harness();let canceled=false;const p=h.run('api("/api/v1/preview")');h.calls[0].resolve({ok:true,status:200,body:{getReader:()=>({read:async()=>({done:false,value:new Uint8Array(1024*1024+1)}),cancel:async()=>{canceled=true;}})}});
  await assert.rejects(p,/oversized/);assert.equal(canceled,true);
 });
 await test('unsaved edits mark preview stale while selected package game survives saving',async()=>{
  const h=harness();h.bindArtifact();h.run('state.preview={state:"succeeded",artifact_digest:artifact.artifact_digest,draft_revision:1,snapshot_sha256:artifact.payload_digest};markDirty()');
  assert.equal(h.run('previewIsStale()'),true);const p=h.event('world-form','submit');h.reply(0,snapshot(2));await p;assert.equal(h.element('package-game').value,'expedition');
 });
 await test('known-request reconciliation suppresses duplicates and preserves newer request input',async()=>{
  const h=harness();h.run('state.publicationAuth={configured:true,renewal:"token_file"}');
  const id='1'.repeat(32);h.element('reconcile-request').value=id;
  const p=h.run('reconcilePublication()');await h.run('reconcilePublication()');
  assert.equal(h.calls.length,1);assert.equal(h.calls[0].url,`/api/v1/publications/${id}/reconcile`);
  assert.equal(h.calls[0].options.method,'POST');assert.equal(h.calls[0].options.body,'{}');
  h.element('reconcile-request').value='2'.repeat(32);
  h.reply(0,{request_id:id,artifact_digest:'c'.repeat(64),status:'approved',target:'staging'});await p;
  assert.equal(h.run('state.publications.length'),1);assert.equal(h.element('reconcile-request').value,'2'.repeat(32));
  assert.match(h.run('state.publicationNotice'),/No submission, review or activation/);
 });
 await test('reconciliation rejects invalid ID or disabled credentials before any request',async()=>{
  const h=harness();h.run('state.publicationAuth={configured:true}');
  for(const id of ['../escape','0'.repeat(32),'A'.repeat(32),'']) {h.element('reconcile-request').value=id;await h.run('reconcilePublication()');}
  h.run('state.publicationAuth={configured:false}');h.element('reconcile-request').value='1'.repeat(32);await h.run('reconcilePublication()');
  assert.equal(h.calls.length,0);
 });
 await test('reconciliation identity mismatch and expired credentials preserve local history',async()=>{
  for(const status of [200,401]) {
   const h=harness();h.run('state.publicationAuth={configured:true,renewal:"token_file"}');h.element('reconcile-request').value='1'.repeat(32);
   const p=h.run('reconcilePublication()');h.reply(0,{request_id:'2'.repeat(32),message:'expired'},status);await p;
   assert.equal(h.run('state.publications.length'),0);assert.equal(h.calls.length,1);assert.equal(h.run('state.busy.size'),0);
   assert.equal(h.element('reconcile-request').value,'1'.repeat(32));
  }
 });
 await test('late publication list cannot erase explicitly reconciled history',async()=>{
  const h=harness();h.run('state.publicationAuth={configured:true}');h.element('reconcile-request').value='1'.repeat(32);
  const list=h.run('loadPublications()');const p=h.run('reconcilePublication()');
  h.reply(1,{request_id:'1'.repeat(32),artifact_digest:'c'.repeat(64),status:'requested',target:'staging'});await p;
  h.reply(0,[]);await list;assert.equal(h.run('state.publications.length'),1);
 });
 console.log(`PASS: ${count} Studio workflow fixture cases; not real Rust/PG/operator/browser acceptance`);
})().catch(error=>{console.error(error);process.exit(1);});
