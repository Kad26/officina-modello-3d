import { townhouse } from '../setups.mjs';

export default {
  name: 'themes-layouts',
  title: 'Themes and layouts',
  topic: 'Basics',
  about: 'Classic (the SketchUp-like default) or SketchCraft’s own looks; docked or floating tool bars; your own icons.',
  setup: townhouse,
  async steps(h) {
    const { page } = h;
    await page.locator('.views button', { hasText: 'Notes' }).click(); // labels off: a cleaner picture
    await page.locator('button', { hasText: /^\s*Iso\s*$/i }).first().click();
    await page.waitForTimeout(500);
    await page.locator('button', { hasText: /^\s*Fit\s*$/i }).first().click();
    await page.waitForTimeout(800);
    await h.caption('Classic: neutral greys and docked tool bars, the default');
    await h.shot(1600);
    await h.caption('Themes, top right: the colors, the layout and the icons');
    await h.click(page.getByRole('button', { name: 'Themes' }), { after: 1200 });
    await h.caption('Workshop: SketchCraft’s own look, with floating tool bars');
    await h.click(page.locator('.skin-card').filter({ has: page.locator('.name', { hasText: /^Workshop$/ }) }));
    await h.click(page.getByRole('button', { name: 'Done' }), { after: 1600 });
    await h.caption('Midnight, for late nights');
    await h.click(page.getByRole('button', { name: 'Themes' }));
    await h.click(page.locator('.skin-card').filter({ has: page.locator('.name', { hasText: /^Midnight$/ }) }));
    await h.caption('Layout on its own: dock the tool bars, keep the colors');
    await h.click(page.getByRole('radio', { name: /Classic/ }));
    await h.click(page.getByRole('button', { name: 'Done' }), { after: 1600 });
    await h.caption('Edit any theme’s colors, load your own icons, share it as a file');
    await h.click(page.getByRole('button', { name: 'Themes' }));
    await h.click(page.locator('.skin-card').filter({ has: page.locator('.name', { hasText: /^Classic$/ }) }));
    await h.click(page.getByRole('button', { name: 'Done' }), { after: 1200 });
  },
};
