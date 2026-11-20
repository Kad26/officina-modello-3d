import { blank } from '../setups.mjs';

export default {
  name: 'first-model',
  title: 'Your first model',
  topic: 'Basics',
  about: 'Draw a rectangle, pull it up, type exact sizes, and look around with the axis gizmo.',
  setup: blank,
  async steps(h) {
    const { page } = h;
    // a close three-quarter view of the ground around the origin
    await page.evaluate(() => {
      const T = window.__sketchcraftView.current;
      T.controls.target.set(2, 1, 0.8);
      T.camera.position.set(10, -8, 6.5);
      T.controls.update();
    });
    await page.waitForTimeout(700);
    await h.caption('A new model: a person stands at the origin for scale');
    await h.shot(1600);
    await h.caption('R for Rectangle, then click one corner on the ground');
    await h.key('r', 'Rectangle');
    const a = await h.at([1, -1, 0]);
    await h.clickAt(...a);
    await h.caption('Move to size it…');
    const b = await h.at([4.5, 1.5, 0]);
    await h.glide(...b, { frames: 12, from: a });
    await h.caption('…or just type the size: 5,4 then Enter');
    await h.type('5,4');
    await h.caption('P for Push/Pull: click the face and pull it up');
    await h.key('p', 'Push/Pull');
    const mid = await h.at([3.5, 1, 0]);
    await h.clickAt(...mid);
    const up = await h.at([3.5, 1, 2.2]);
    await h.glide(...up, { frames: 12, from: mid });
    await h.caption('Type the height: 3 then Enter');
    await h.type('3');
    await h.caption('Drag the axis gizmo (top right) to look around');
    const g = await page.locator('.axis-gizmo').boundingBox();
    const c = [g.x + g.width / 2, g.y + g.height / 2 + 10];
    await page.mouse.move(...c);
    await page.mouse.down();
    await h.glide(c[0] + 70, c[1], { frames: 14, from: c });
    await page.mouse.up();
    await h.caption('Click an axis to look straight along it: Z is the top view');
    const z = await page.locator('[data-axis="+z"]').boundingBox();
    await h.clickAt(z.x + z.width / 2, z.y + z.height / 2, { after: 300 });
    await page.waitForTimeout(600); // the camera glides there
    await h.shot(1200);
    await page.locator('button', { hasText: /^\s*Iso\s*$/i }).first().click();
    await page.waitForTimeout(500);
    await page.locator('button', { hasText: /^\s*Fit\s*$/i }).first().click();
    await page.waitForTimeout(900);
    await h.caption('Your first model. ⌘Z undoes anything.');
    await h.shot(1600);
  },
};
