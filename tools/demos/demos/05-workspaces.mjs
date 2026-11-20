import { townhouse } from '../setups.mjs';

export default {
  name: 'workspaces',
  title: 'Workspaces',
  topic: 'Basics',
  about: 'Model, Arch, Design and Previz arrange the tools for the job: walls and plans, solids, scenes and renders.',
  setup: townhouse,
  async steps(h) {
    const { page } = h;
    await page.locator('.views button', { hasText: 'Notes' }).click();
    await h.caption('Workspaces arrange the app for the job');
    await h.click(page.getByRole('tab', { name: 'Arch' }));
    await h.caption('Arch: walls, stairs and roofs, plans by floor, blueprints');
    await h.click(page.locator('.ws-strip button', { hasText: 'Upper' }), { after: 1500 });
    await h.click(page.locator('.ws-strip button', { hasText: '3D' }));
    await h.caption('Design: solids, bevels and rounds');
    await h.click(page.getByRole('tab', { name: 'Design' }));
    await h.caption('Previz: scenes, sun and photoreal renders');
    await h.click(page.getByRole('tab', { name: 'Previz' }));
    await h.click(page.getByRole('tab', { name: 'Model' }));
  },
};
