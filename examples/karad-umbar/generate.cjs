'use strict';
// Offline edition compiler: no runtime RNG, database access or live world edits.
const {createHash} = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const base = require('./world.json');
const halls = [['c','cup','water keeper'],['b','cradle','bell founder'],['f','seed','spore farmer'],['l','car','lift tender'],['a','blank','copyist'],['s','bowl','salt carrier'],['k','urn','potter'],['h','cradle','ward nurse'],['v','heart','assayer']];
const signs = ['north','east','west'];
function edition(seed) {
  if (typeof seed !== 'string' || !/^[a-z0-9-]{1,48}$/.test(seed)) throw Error('Use 1–48 lowercase letters, digits or hyphens.');
  const w = structuredClone(base), rooms = new Map(w.rooms.map(r=>[r.id,r]));
  const get = id => rooms.get('umbar:'+id);
  // Label-separated hash ranks keep unrelated design changes from consuming another draw.
  const rank = (group,values) => values.map(value=>({value,key:createHash('sha256').update(JSON.stringify(['karad-umbar-v1',seed,group,value])).digest('hex')})).sort((a,b)=>a.key<b.key?-1:a.key>b.key?1:0).map(x=>x.value);
  const witnesses=rank('witnesses',halls).slice(0,2), marks=rank('marks',signs);
  const clues=witnesses.map(([district,site,job],i)=>{
    const station=get(district+'.'+site); station.description+=` A shift token stamped for the ${job} reads: ${marks[i]}.`;
    return `the ${job} at ${station.title} (off the ring at ${get(district+'.gate').title})`;
  });
  get('rooms.slate').description=`First witness: ${clues[0]}. Second witness: ${clues[1]}. Find both shift tokens. Meet back here and take the marked passages in order.`;
  for (let i=0;i<2;i++) {
    const seal=get(i?'rooms.second':'rooms.first'); const wrong=signs.filter(x=>x!==marks[i]);
    seal.exits={[marks[i]]:get(i?'rooms.roll':'rooms.second').id,[wrong[0]]:get('rooms.muster').id,[wrong[1]]:get('rooms.slate').id};
  }
  get('rooms.roll').description=`The copper roll proves it: the ${witnesses[0][2]} led the children out; the ${witnesses[1][2]} stayed to seal the gate. No royal name appears on the last shift.`;
  return w;
}
module.exports={edition};
if(require.main===module){const [seed,out,...more]=process.argv.slice(2);if(!out||more.length)throw Error('usage: node generate.cjs SEED NEW-FILE.json');fs.writeFileSync(path.resolve(out),JSON.stringify(edition(seed),null,2)+'\n',{flag:'wx',mode:0o600});}
