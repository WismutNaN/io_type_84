import { test, expect } from '@playwright/test';
import { fixture } from '../fixture';
test('all 84 keys, selection, depth actions, languages and themes fit supported sizes', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.setViewportSize({ width: 1480, height: 908 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Открыть пример', exact: true }).click();
  await page.locator('[data-slot="105"]').click();
  await page.getByRole('button', { name: 'Сохранить действие', exact: true }).click();
  await expect(page.locator('[data-slot="105"]')).toContainText('Vol +');
  await page.locator('[data-slot="108"]').click();
  await page.getByRole('button', { name: 'Сохранить действие', exact: true }).click();
  await expect(page.locator('[data-slot="108"]')).toContainText('Vol −');
  for (const size of [
    { width: 1480, height: 908 },
    { width: 960, height: 668 },
  ]) {
    await page.setViewportSize(size);
    await expect(page.locator('.keycap')).toHaveCount(84);
    const outside = await page.locator('.keycap').evaluateAll((keys) =>
      keys
        .filter((k) => {
          const r = k.getBoundingClientRect();
          return r.left < 0 || r.right > innerWidth || r.top < 0 || r.bottom > innerHeight;
        })
        .map((k) => k.getAttribute('data-slot')),
    );
    expect(outside).toEqual([]);
    await expect(page.getByRole('button', { name: 'Применить', exact: true })).toBeInViewport();
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
    expect(overflow).toBe(false);
  }
  await page.locator('.editor-scroll').evaluate((e) => e.scrollTo({ top: 0 }));
  await page.screenshot({ path: 'archive_data/ui-960-light-ru.png' });
  await page.getByRole('button', { name: 'Параметры', exact: true }).click();
  await page.getByRole('combobox', { name: 'Язык', exact: true }).selectOption('en');
  await page.getByRole('combobox', { name: 'Appearance', exact: true }).selectOption('dark');
  await page.getByRole('button', { name: 'Keyboard', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Save action', exact: true })).toBeVisible();
  await page.screenshot({ path: 'archive_data/ui-960-dark-en.png' });
  await page.setViewportSize({ width: 1480, height: 908 });
  await page.screenshot({ path: 'archive_data/ui-1480-dark-en.png' });
  for (const label of ['Actuation', 'Behavior']) {
    await page.getByRole('button', { name: label, exact: true }).click();
    const body = await page.locator('.editor-scroll').innerText();
    expect(body).not.toMatch(/[А-Яа-яЁё]/);
  }
  for (const label of ['Lighting', 'Catalog', 'Profiles', 'Settings']) {
    await page.getByRole('button', { name: label, exact: true }).click();
    await page.screenshot({ path: `archive_data/ui-${label.toLowerCase()}-en.png` });
  }
  expect(errors).toEqual([]);
});

test('release and a stalled IPC frame clear movement without clearing selection', async ({
  page,
}) => {
  const snapshot = fixture();
  snapshot.keys[105]!.base = { page: 2, parameters: [0, 75, 0] };
  await page.addInitScript(
    ({ snapshot }) => {
      const w = window as any;
      w.isTauri = true;
      w.testFrame = {
        active: true,
        travel: [],
        history: [],
        colors: [],
        colorAgeMs: null,
        packets: 0,
        message: null,
        rulesEnabled: false,
        ruleFirings: 0,
        ruleError: null,
      };
      w.stall = false;
      w.__TAURI_INTERNALS__ = {
        invoke: async (cmd: string) => {
          if (cmd === 'connect_device') return snapshot;
          if (cmd === 'monitor_frame' || cmd === 'set_monitor') {
            if (w.stall) return new Promise(() => {});
            return structuredClone(w.testFrame);
          }
          return null;
        },
      };
    },
    { snapshot },
  );
  await page.setViewportSize({ width: 1480, height: 908 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Подключить', exact: true }).first().click();
  await expect(page.locator('[data-slot="105"] .key-label')).toHaveText('PgUp');
  const key = page.locator('[data-slot="49"]');
  await key.click();
  const sample = { slot: 49, travelUm: 2100, maxTravelUm: 3200, adc: 400, ageMs: 0 };
  await page.evaluate((sample) => {
    (window as any).testFrame.travel = [sample];
  }, sample);
  await expect(key).toHaveClass(/pressed/);
  await expect(key).toHaveClass(/selected/);
  const visual = await key.evaluate((e) => ({
    outline: getComputedStyle(e).outlineStyle,
    fill: getComputedStyle(e.querySelector('.key-travel-fill')!).width,
  }));
  expect(visual.outline).toBe('solid');
  expect(parseFloat(visual.fill)).toBeGreaterThan(1);
  await page.evaluate(() => {
    (window as any).testFrame.travel[0].travelUm = 0;
  });
  await expect(key).not.toHaveClass(/pressed/);
  await expect(key).toHaveClass(/selected/);
  await page.evaluate(() => {
    (window as any).testFrame.travel[0].travelUm = 2500;
  });
  await expect(key).toHaveClass(/pressed/);
  await page.evaluate(() => {
    (window as any).stall = false;
    (window as any).testFrame.history = Array.from({ length: 80 }, (_, i) => ({ sequence: 80-i, slot: 105, peakUm: 3100 }));
  });
  await expect(page.locator('.press-history button').first()).toBeVisible();
  const history = await page.locator('.press-history').evaluate(e => ({ count: e.children.length, width: e.clientWidth, overflow: getComputedStyle(e).overflowX, scroll: e.scrollWidth }));
  expect(history.count).toBeLessThanOrEqual(Math.floor((history.width+5)/89));
  expect(history.overflow).toBe('hidden');
  expect(history.scroll).toBeLessThanOrEqual(history.width);
  await page.evaluate(() => {
    (window as any).stall = true;
  });
  await expect(key).not.toHaveClass(/pressed/, { timeout: 2000 });
  await expect(key).toHaveClass(/selected/);
  await expect(page.locator('.inspection-travel output')).toContainText('—');

});

test('lighting previews motion, clearing groups and layout match keyboard', async ({ page }) => {
  await page.setViewportSize({ width: 1480, height: 908 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Открыть пример', exact: true }).click();
  await page.getByRole('button', { name: 'Все', exact: true }).click();
  await expect(page.locator('.keycap.selected')).toHaveCount(84);
  await page.getByRole('button', { name: 'Снять выделение', exact: true }).click();
  await expect(page.locator('.keycap.selected')).toHaveCount(0);
  await page.getByRole('button', { name: 'Свет', exact: true }).click();
  await page.getByRole('button', { name: 'Движение', exact: true }).click();
  await page.getByRole('button', { name: 'Волна', exact: true }).click();
  const key = page.locator('[data-slot="105"]');
  const initial = await key.evaluate(e => (e as HTMLElement).style.getPropertyValue('--key-color'));
  await expect.poll(() => key.evaluate(e => (e as HTMLElement).style.getPropertyValue('--key-color'))).not.toBe(initial);
  await expect(page.locator('.preview-caption')).toContainText('Визуальная модель');
  for (const width of [1480, 960]) {
    await page.setViewportSize({ width, height: 800 });
    const board = (await page.locator('.keyboard-area').boundingBox())!;
    const editor = (await page.locator('.editor-scroll').boundingBox())!;
    if (width >= 1280) expect(editor.x).toBeGreaterThanOrEqual(board.x + board.width - 1);
    else expect(editor.y).toBeGreaterThanOrEqual(board.y + board.height - 1);
    await page.screenshot({ path: `archive_data/ui-light-preview-${width}.png` });
  }
});

test('every editor reflows at minimum size, numeric depth entry is optional', async ({ page }) => {
  await page.setViewportSize({ width: 960, height: 668 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Открыть пример', exact: true }).click();
  await page.locator('[data-slot="105"]').click();
  await page.getByRole('button', { name: 'Срабатывание', exact: true }).click();
  await page.getByRole('button', { name: 'Глубокое', exact: true }).click();
  await page.getByRole('button', { name: 'Добавить в черновик', exact: true }).click();
  await expect(page.locator('.draft-summary')).toContainText('1');
  await page.getByRole('button', { name: 'Отменить', exact: true }).click();
  await expect(page.getByRole('slider', { name: 'Точка срабатывания', exact: true })).toHaveValue(
    '1200',
  );
  await page.getByRole('button', { name: 'Поведение', exact: true }).click();
  await page.getByRole('button', { name: 'Глубокое нажатие', exact: false }).last().click();
  await expect(page.locator('.dks-editor input[type="number"]')).toHaveCount(0);
  for (const nav of ['Свет', 'Каталог', 'Профили', 'Параметры']) {
    await page.getByRole('button', { name: nav, exact: true }).click();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(960);
    const clipped = await page.locator('button:visible,select:visible').evaluateAll((elements) =>
      elements
        .filter((e) => {
          const r = e.getBoundingClientRect();
          return r.left < 0 || r.right > innerWidth;
        })
        .map((e) => e.textContent),
    );
    expect(clipped).toEqual([]);
  }
});

test('gestures share the draft, undo, profile and native apply flow', async ({page}) => {
  await page.addInitScript(({snapshot})=>{
    const w=window as any; w.isTauri=true; w.calls=[];
    const frame={active:false,travel:[],history:[],colors:[],colorAgeMs:null,packets:0,message:null,rulesEnabled:false,ruleFirings:0,ruleError:null};
    w.__TAURI_INTERNALS__={invoke:async(cmd:string,args:any)=>{
      if(cmd!=='monitor_frame')w.calls.push({cmd,args});
      if(cmd==='connect_device')return snapshot;
      if(cmd==='set_monitor'){frame.active=args.enabled;return structuredClone(frame);}
      if(cmd==='configure_automation'){frame.rulesEnabled=args.enabled; frame.active=args.enabled;}
      if(cmd==='prepare_automation')return {token:'ownership-preview', changes:[{block:'base',changedBytes:4},{block:'function',changedBytes:4}]};
      if(cmd==='monitor_frame')return structuredClone(frame);
      return null;
    }};
  },{snapshot:fixture()});
  await page.setViewportSize({width:1480,height:908});await page.goto('/');
  await page.getByRole('button',{name:'Подключить',exact:true}).first().click();
  await page.locator('[data-slot="105"]').click();
  await page.getByRole('button',{name:'Сохранить действие',exact:true}).click();
  await expect(page.locator('.draft-summary')).toContainText('1');
  await page.getByRole('button',{name:'Отменить',exact:true}).click();
  await expect(page.locator('[data-slot="105"] .key-binding')).toHaveCount(0);
  await page.getByRole('button',{name:'Повторить',exact:true}).click();
  await expect(page.locator('[data-slot="105"]')).toContainText('Vol +');
  await page.getByRole('button',{name:'Снять выделение',exact:true}).click();
  await page.locator('[data-slot="49"]').click();
  await page.locator('[data-slot="50"]').click({modifiers:['Control']});
  await page.getByRole('switch',{name:'Удерживать',exact:true}).check();
  await page.getByRole('combobox',{name:'Действие жеста',exact:true}).selectOption('word');
  await page.getByRole('button',{name:'Сохранить действие',exact:true}).click();
  await page.getByRole('button',{name:'Сохранить профиль',exact:true}).click();
  const saved=await page.evaluate(()=>JSON.parse(localStorage.getItem('io.profiles.v1')!)[0]);
  expect(saved.schemaVersion).toBe(2);expect(saved.automation.gestures).toHaveLength(1);expect(saved.automation.depthChoices).toHaveLength(1);
  expect(saved.automation.gestures[0].slots).toEqual([49,50]);
  await page.getByRole('button',{name:'Применить',exact:true}).click();
  await expect(page.getByRole('dialog')).toContainText('Word');
  await expect(page.getByRole('dialog')).toContainText('Передать клавиши приложению');
  expect((await page.evaluate(()=>(window as any).calls)).some((c:any)=>c.cmd==='configure_automation')).toBe(false);
  await page.getByRole('button',{name:'Сохранить на компьютере',exact:true}).click();
  await expect(page.locator('.draft-summary')).toContainText('Жесты компьютера включены');
  const calls=await page.evaluate(()=>(window as any).calls);
  expect(calls.some((c:any)=>c.cmd==='apply_changes'||c.cmd==='prepare_changes')).toBe(false);
  const applied=calls.find((c:any)=>c.cmd==='configure_automation');
  expect(applied.args.enabled).toBe(true);expect(applied.args.profile.gestures).toHaveLength(1);expect(applied.args.profile.depthChoices).toHaveLength(1);
  expect(applied.args.ownershipToken).toBe('ownership-preview');
  await page.getByRole('button',{name:'Каталог',exact:true}).click();
  await page.getByRole('button',{name:'Новое действие',exact:false}).click();
  await page.getByRole('textbox',{name:'Название действия',exact:true}).fill('Мой сценарий');
  await page.getByRole('combobox',{name:'Тип действия',exact:true}).selectOption('macro');
  await page.getByRole('button',{name:'Добавить шаг',exact:false}).click();
  await page.getByRole('combobox',{name:'Тип действия',exact:true}).last().selectOption('text');
  await page.getByRole('textbox',{name:'Текст',exact:true}).fill('Hello');
  await page.getByRole('button',{name:'Добавить в черновик',exact:true}).click();
  await expect(page.locator('.catalog-list')).toContainText('Мой сценарий');
  await page.locator('.page-scroll').evaluate(e=>e.scrollTo({top:0}));
  await page.screenshot({path:'archive_data/ui-action-catalog.png'});
});
