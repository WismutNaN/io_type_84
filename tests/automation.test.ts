import { test } from 'node:test';
import assert from 'node:assert/strict';
import { defaultAutomation, migrateDepthRules, readAutomation } from '../src/features/editor/computer-rules.ts';
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
