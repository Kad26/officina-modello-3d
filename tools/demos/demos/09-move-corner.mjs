import { plainBox } from '../setups.mjs';

export default {
  name: 'move-corner',
  title: 'Move a single corner',
  topic: 'Shape',
  about: 'With nothing selected, Move grabs the corner under the cursor; the faces around it fold to stay flat.',
  setup: plainBox,
  async steps(h) {
    const { page } = h;
    await page.locator('button', { hasText: /^\s*Iso\s*$/i }).first().click();
    await page.waitForTimeout(700);
    await h.caption('M for Move, with nothing selected');
    await h.key('m', 'Move');
    // the top corner nearest the camera
    const corner = await page.evaluate(() => {
      const T = window.__sketchcraftView.current;
      const cs = [[0, 0, 2.6], [4, 0, 2.6], [4, 3, 2.6], [0, 3, 2.6]];
      return cs.sort((a, b) => T.camera.position.distanceTo({ x: a[0], y: a[1], z: a[2] }) - T.camera.position.distanceTo({ x: b[0], y: b[1], z: b[2] }))[0];
    });
    const p = await h.at(corner);
    await h.caption('Click a corner: just that point is picked up');
    await page.mouse.move(p[0] + 3, p[1] + 3);
    await h.clickAt(...p);
    const q = await h.at([corner[0], corner[1], corner[2] + 1.4]);
    await h.caption('Its edges stretch as you move');
    await h.glide(...q, { frames: 14, from: p });
    await h.caption('Type the distance: 1.5 then Enter');
    await h.type('1.5', { after: 1200 });
    await h.caption('The bent faces fold so every face stays flat');
    await h.shot(1800);
  },
};
