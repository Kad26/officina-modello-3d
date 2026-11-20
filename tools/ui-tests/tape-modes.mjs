#!/usr/bin/env node
// Run against the VITE_OFFICINA=1 dev page and a bridge using the same scratch SKETCHCRAFT_DOC.
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
  for (const command of [{ cmd: 'new' }, { cmd: 'rect', from: [0, 0, 0], to: [4, 4, 0] }]) {
    const reply = await pageExec(page, command);
    assert(reply.ok, reply.error);
  }
  await page.reload();
  await page.waitForSelector('.viewport canvas');
  await page.waitForFunction(() => {
    const target = window.__sketchcraftView?.current?.controls?.target;
    return target && Math.abs(target.x - 2) < 0.05 && Math.abs(target.y - 2) < 0.05;
  });
  const screen = (...world) => page.evaluate((world) => {
    const T = window.__sketchcraftView.current;
    const point = T.camera.position.clone().set(...world).project(T.camera);
    const r = T.renderer.domElement.getBoundingClientRect();
    return [r.left + (point.x + 1) * r.width / 2, r.top + (1 - point.y) * r.height / 2];
  }, world);
  const waitMode = (name) => page.locator('label[for=measure-value]', { hasText: name }).waitFor();
  const waitGuides = async (count) => {
    for (let i = 0; i < 30; i++) {
      const { guides } = await pageScene(page);
      if (guides.length === count) return guides;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.fail(`expected ${count} guides`);
  };
  const measurement = page.locator('#measure-value');
  const submit = async (value) => {
    await measurement.fill(value);
    await measurement.press('Enter');
  };
  const click = async (...p) => page.mouse.click(...(await screen(...p)));
  const ctrl = () => page.keyboard.press('Control');

  await page.keyboard.press('t');
  await waitMode(/Guide Line|Linea guida/);
  await click(2, 0, 0);
  await page.mouse.move(...(await screen(3.3, 1, 0)));
  await submit('1.25m');
  const [line] = await waitGuides(1);
  assert(Math.abs(line.p[0] - 2) < 0.001 && Math.abs(line.p[1] - 1.25) < 0.02 && Math.abs(line.p[2]) < 0.02,
    'typed distance offsets perpendicular to the source line');
  assert(Math.abs(line.dir[0]) === 1 && line.dir[1] === 0 && line.dir[2] === 0, 'guide is parallel to the source edge');

  await page.keyboard.press('Control+c');
  await waitMode(/Guide Line|Linea guida/);
  await measurement.focus();
  await ctrl();
  await waitMode(/Guide Line|Linea guida/);
  await measurement.press('Escape');

  await ctrl();
  await waitMode(/Guide Point|Punto guida/);
  await click(4, 4, 0);
  const [, point] = await waitGuides(2);
  assert.deepEqual(point.p, [4, 4, 0], 'guide point snaps to the existing corner');
  assert(point.dir == null, 'point has no line direction');

  await ctrl();
  await waitMode(/^Measure$|^Misura$/);
  await click(0, 0, 0);
  await click(4, 0, 0);
  await page.waitForFunction(() => /^4\D/.test(document.querySelector('#measure-value')?.placeholder ?? ''));
  assert.equal((await pageScene(page)).guides.length, 2, 'measuring does not create a guide');
  await submit('8m');
  const resize = page.locator('.sheet-backdrop', { hasText: /Resize the model|Ridimensiona/ });
  await resize.waitFor();
  await page.keyboard.press('Escape');
  await resize.waitFor({ state: 'hidden' });
  await ctrl();
  await waitMode(/Guide Line|Linea guida/);

  console.log('Windows tape modes: Ctrl cycle, exact line offset, inferred point, read-only measurement and resize prompt passed');
} finally {
  await browser.close();
}
