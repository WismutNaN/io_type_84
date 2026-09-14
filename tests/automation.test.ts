import { test } from 'node:test';
import assert from 'node:assert/strict';
import { defaultAutomation, migrateDepthRules, readAutomation, pageDepthChoice } from '../src/features/editor/computer-rules.ts';
import { readLocalProfile } from '../src/features/editor/model.ts';
import { fixture } from './fixture.ts';
test('old depth rules migrate into reusable actions; missing targets and nested payloads fail', () => {
  const p=migrateDepthRules([{slot:105,thresholdUm:2400,releaseUm:1800,action:'volumeUp'}]);
  assert.equal(p.gestures[0]!.actionId,'volumeUp');
  p.gestures.push({...p.gestures[0]!,id:'full',thresholdUm:3000});
  assert.equal(readAutomation(p).gestures.length,2);
  p.gestures[0]!.actionId='missing'; assert.throws(()=>readAutomation(p));
  const invalid=defaultAutomation();
  invalid.actions[0]!.platformCommands.linux={kind:'macro',steps:[{kind:'delay',ms:65535}]};
  assert.throws(()=>readAutomation(invalid));
});
test('profile v2 round trips gestures and platform overrides; v1 migration is inert', () => {
  const automation=defaultAutomation();
  automation.actions[0]!.platformCommands.macos={kind:'key',key:43,modifiers:8};
  const v2={schemaVersion:2,name:'Test',savedAt:new Date().toISOString(),snapshot:fixture(),automation};
  assert.deepEqual(readLocalProfile(JSON.stringify(v2)).automation,automation);
  const v1={...v2,schemaVersion:1,automation:undefined};
  assert.equal(readLocalProfile(JSON.stringify(v1)).automation!.gestures.length,0);
});


test('exclusive depth references survive export; conflicts and invalid thresholds are rejected', () => {
  const legacy = defaultAutomation();
  legacy.gestures.push({ id:'old',slots:[105],thresholdUm:2400,releaseUm:1800,holdMs:0,actionId:'volumeUp' });
  const p = pageDepthChoice(legacy, 105, 'volumeUp');
  assert.equal(p.gestures.length,0);
  assert.equal(legacy.gestures.length,1);
  assert.equal(p.depthChoices[0]!.lightActionId,'page-105');
  const exported = readLocalProfile(JSON.stringify({schemaVersion:2,name:'Depth',savedAt:new Date().toISOString(),snapshot:fixture(),automation:p}));
  assert.deepEqual(exported.automation,p);
  const bad = structuredClone(p); bad.depthChoices[0]!.deepUm = 500; assert.throws(()=>readAutomation(bad));
  bad.depthChoices[0]!.deepUm = 3000; bad.gestures = legacy.gestures; assert.throws(()=>readAutomation(bad));
  const missing = structuredClone(p); missing.actions = missing.actions.filter(a=>a.id!=='page-105'); assert.throws(()=>readAutomation(missing));
  const old:any = defaultAutomation(); delete old.depthChoices;
  assert.deepEqual(readAutomation(old).depthChoices,[]);
});

test('repeat defaults only for new short actions; old choices and explicit off remain single-shot', () => {
  const p = pageDepthChoice(defaultAutomation(),105,'volumeUp');
  assert.deepEqual(p.depthChoices[0]!.deepRepeat,{delayMs:350,intervalMs:80});
  const legacy:any = structuredClone(p); delete legacy.depthChoices[0].deepRepeat;
  const migrated = readAutomation(legacy);
  assert.equal(migrated.depthChoices[0]!.deepRepeat,null);
  assert.equal(pageDepthChoice(migrated,105,'volumeUp').depthChoices[0]!.deepRepeat,null);
  const off = pageDepthChoice(p,105,'volumeUp',3000,undefined,600,null);
  assert.equal(off.depthChoices[0]!.deepRepeat,null);
  assert.equal(pageDepthChoice(defaultAutomation(),108,'word').depthChoices[0]!.deepRepeat,null);
  const custom = pageDepthChoice(off,105,'volumeUp',3000,undefined,600,{delayMs:1200,intervalMs:200});
  assert.deepEqual(readAutomation(custom),custom);
});

test('repeat rejects invalid timing and toggles, macros or launches in any OS variant', () => {
  const p = pageDepthChoice(defaultAutomation(),105,'volumeUp');
  for (const value of [false,{},[],{delayMs:99,intervalMs:80},{delayMs:350,intervalMs:0},{delayMs:350,intervalMs:80.5}]) {
    const bad:any = structuredClone(p); bad.depthChoices[0].deepRepeat = value;
    assert.throws(()=>readAutomation(bad));
  }
  for (const command of [{kind:'media',action:'mute'},{kind:'application',application:'word'},{kind:'macro',steps:[{kind:'delay',ms:100}]}]) {
    const bad:any = structuredClone(p); bad.actions[0].platformCommands.linux = command;
    assert.throws(()=>readAutomation(bad));
  }
});
