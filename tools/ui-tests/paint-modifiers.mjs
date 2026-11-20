#!/usr/bin/env node
// Run against the VITE_OFFICINA=1 dev page and a current bridge on a scratch SKETCHCRAFT_DOC.
import assert from 'node:assert/strict';
import { launch, PAGE, pageExec, pageScene } from '../demos/lib.mjs';

if (!process.env.SKETCHCRAFT_DOC) throw new Error('Set SKETCHCRAFT_DOC to a scratch model path.');

const browser = await launch();
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'platform', { value: 'Win32' });
    localStorage.setItem('sketchcraft.layout', 'side');
  });
  await page.goto(PAGE);
  const X = async (command) => {
    const reply = await pageExec(page, command);
    assert(reply.ok, `${JSON.stringify(command)}: ${reply.error}`);
    return reply;
  };
  const rect = async (x, y, material) => {
    const { face } = await X({ cmd: 'rect', from: [x, y, 0], to: [x + 2, y + 2, 0] });
    await X({ cmd: 'paint', ids: [face], material });
    return face;
  };

  await X({ cmd: 'new' });
  await X({ cmd: 'material', name: 'Paint Red', color: [0.9, 0.04, 0.02] });
  await X({ cmd: 'material', name: 'Paint Blue', color: [0.05, 0.2, 0.9], roughness: 0.31 });
  await X({ cmd: 'material', name: 'Paint Yellow', color: [0.9, 0.7, 0.03] });
  const a = await rect(0, 0, 'Paint Red');
  const b = await rect(2, 0, 'Paint Red');
  const bridgeFace = await rect(4, 0, 'Paint Yellow');
  const c = await rect(6, 0, 'Paint Red');
  const sample = await rect(10, 0, 'Paint Blue');
  const groupA = await rect(0, 5, 'Paint Red');
  const groupB = await rect(3, 5, 'Paint Red');
  const groupYellow = await rect(6, 5, 'Paint Yellow');
  await X({ cmd: 'group', ids: [groupA, groupB, groupYellow] });
  const otherGroupFace = await rect(9, 5, 'Paint Red');
  await X({ cmd: 'group', ids: [otherGroupFace] });
  const componentFace = await rect(15, 5, 'Paint Red');
  const { face: componentDefault } = await X({ cmd: 'rect', from: [18, 5, 0], to: [20, 7, 0] });
  const { instance } = await X({ cmd: 'make_component', ids: [componentFace, componentDefault], name: 'Closed component' });

  await page.reload();
  await page.waitForSelector('.viewport canvas');
  await page.waitForFunction(() => {
    const target = window.__sketchcraftView?.current?.controls?.target;
    return target && Math.abs(target.x - 10) < 0.1 && Math.abs(target.y - 3.5) < 0.1;
  });
  const click = async (x, y, ...modifiers) => {
    const point = await page.evaluate(([x, y]) => {
      const T = window.__sketchcraftView.current;
      const v = T.camera.position.clone().set(x, y, 0).project(T.camera);
      const r = T.renderer.domElement.getBoundingClientRect();
      return [r.left + (v.x + 1) * r.width / 2, r.top + (1 - v.y) * r.height / 2];
    }, [x, y]);
    for (const key of modifiers) await page.keyboard.down(key);
    await page.mouse.click(...point);
    for (const key of modifiers.reverse()) await page.keyboard.up(key);
  };
  const materials = (scene, ids) => ids.map((id) => scene.faces.find((face) => face.id === id)?.material);
  const waitMaterial = async (target, id) => {
    for (let i = 0; i < 30; i++) {
      const scene = await pageScene(page);
      if (materials(scene, [id])[0] === target) return scene;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.fail(`expected face ${id} to use ${target}`);
  };

  await page.keyboard.press('b');
  await click(11, 1, 'Alt');
  await click(1, 1, 'Control');
  let scene = await waitMaterial('Paint Blue', a);
  assert.deepEqual(materials(scene, [b, bridgeFace, c, sample]), ['Paint Blue', 'Paint Yellow', 'Paint Red', 'Paint Blue']);

  await click(1, 6, 'Control', 'Shift');
  scene = await waitMaterial('Paint Blue', groupA);
  assert.deepEqual(materials(scene, [groupB, groupYellow, otherGroupFace, c]), ['Paint Blue', 'Paint Yellow', 'Paint Red', 'Paint Red']);

  await click(7, 1, 'Shift');
  scene = await waitMaterial('Paint Blue', c);
  assert.deepEqual(materials(scene, [otherGroupFace, bridgeFace, groupYellow]), ['Paint Blue', 'Paint Yellow', 'Paint Yellow']);
  const componentTri = scene.triFace.indexOf(instance);
  assert.equal(scene.paint.materials[scene.paint.triMaterial[componentTri]], 'Paint Blue');
  console.log('Windows Paint Bucket: Alt retains the named material, Ctrl follows matching connected faces, Ctrl+Shift limits the object, Shift includes closed components');

  await click(5, 1, 'Alt');
  await click(19, 6);
  let rendered;
  for (let i = 0; i < 30; i++) {
    scene = await pageScene(page);
    rendered = new Set(scene.triFace.flatMap((owner, index) => owner === instance ? [scene.paint.materials[scene.paint.triMaterial[index]]] : []));
    if (rendered.has('Paint Yellow')) break;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  assert.deepEqual(rendered, new Set(['Paint Blue', 'Paint Yellow']));
  console.log('Windows plain Paint Bucket: closed component inherits the chosen material and preserves explicit face paint');
} finally {
  await browser.close();
}
