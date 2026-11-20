#!/usr/bin/env node
// Run against the dev page and a bridge using the same scratch SKETCHCRAFT_DOC.
// VITE_OFFICINA=1 exercises the inset viewport of the desktop layout.
import assert from 'node:assert/strict';
import { launch, PAGE, pageExec, pageScene } from '../demos/lib.mjs';

if (!process.env.SKETCHCRAFT_DOC) throw new Error('Set SKETCHCRAFT_DOC to a scratch model path.');

const browser = await launch();
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  await page.addInitScript(() => localStorage.setItem('sketchcraft.layout', 'side'));
  await page.goto(PAGE);
  const X = async (command) => {
    const reply = await pageExec(page, command);
    assert(reply.ok, reply.error);
    return reply;
  };
  await X({ cmd: 'new' });
  const rect = (x) => X({ cmd: 'rect', from: [x, 0, 0], to: [x + 4, 4, 0] });
  const group = async (x) => (await X({ cmd: 'group', ids: [(await rect(x)).face] })).group;
  const firstGroup = await group(0);
  await group(6);
  await rect(12);
  await page.reload();
  const canvas = page.locator('.viewport canvas');
  await canvas.waitFor();
  await page.waitForFunction(() => window.__sketchcraftView?.current?.camera);
  await page.keyboard.press('Space');

  const screen = (...world) => page.evaluate((world) => {
    const T = window.__sketchcraftView.current;
    const point = T.camera.position.clone().set(...world).project(T.camera);
    const r = T.renderer.domElement.getBoundingClientRect();
    return [r.left + (point.x + 1) * r.width / 2, r.top + (1 - point.y) * r.height / 2];
  }, world);
  await page.waitForFunction(() => {
    const T = window.__sketchcraftView.current;
    return Math.abs(T.controls.target.x - 8) < 0.1 && Math.abs(T.controls.target.y - 2) < 0.1;
  });
  const first = await screen(2, 2, 0);
  const second = await screen(8, 2, 0);
  const face = await screen(14, 2, 0);
  const edge = await screen(14, 0, 0);
  const right = (point) => page.mouse.click(...point, { button: 'right' });
  const dismiss = () => page.keyboard.press('Escape');
  const menu = page.locator('.stage .menu');
  const item = (name) => menu.getByRole('button', { name }).first();

  await right(face);
  await item(/Orient faces|Orienta facce/).waitFor();
  await dismiss();
  await right(edge);
  await item(/Divide|Dividi/).waitFor();
  assert.equal(await item(/Orient faces|Orienta facce/).count(), 0);
  await dismiss();

  await page.mouse.click(...first);
  await page.keyboard.down('Control');
  await page.mouse.click(...second);
  await page.keyboard.up('Control');
  await right(first);
  await item(/Solid: Trim|Solidi: ritaglia/).waitFor();
  await dismiss();

  const bounds = await canvas.boundingBox();
  const blank = [bounds.x + 28, bounds.y + 28];
  await right(blank);
  await item(/Unhide all|Mostra tutto/).waitFor();
  const opened = await menu.boundingBox();
  assert(Math.abs(opened.x - blank[0]) <= 2, 'menu must open at the cursor horizontally');
  assert(Math.abs(opened.y - blank[1]) <= 2, 'menu must open at the cursor vertically');
  // On Windows a browser contextmenu event can follow pointerup, after the custom menu opened.
  await page.locator('.menu-scrim').dispatchEvent('contextmenu', { button: 2, clientX: blank[0], clientY: blank[1] });
  assert(await menu.isVisible(), 'native contextmenu event must leave the custom menu open');
  await dismiss();

  const corner = [bounds.x + bounds.width - 12, bounds.y + bounds.height - 12];
  await right(corner);
  await item(/Unhide all|Mostra tutto/).waitFor();
  const fitted = await menu.boundingBox();
  const stage = await page.locator('.stage').boundingBox();
  assert(fitted.x + fitted.width <= stage.x + stage.width, 'menu fits left of the tray');
  assert(fitted.y + fitted.height <= stage.y + stage.height, 'menu fits above the bottom edge');
  await dismiss();

  await page.mouse.move(...blank);
  await page.mouse.down({ button: 'right' });
  await page.mouse.move(blank[0] + 90, blank[1] + 25, { steps: 5 });
  await page.mouse.move(...blank, { steps: 5 });
  await page.mouse.up({ button: 'right' });
  assert.equal(await menu.count(), 0, 'a right drag ending where it started must leave the menu closed');

  await right(await screen(2, 2, 0));
  await item(/Edit group|Modifica gruppo/).click();
  assert.equal((await pageScene(page)).activeGroup, firstGroup, 'menu action should affect the group under the cursor');
  console.log('Viewport context menu: face, edge, group, multiple selection, blank area, position and drag passed');
} finally {
  await browser.close();
}
