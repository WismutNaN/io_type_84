import { readAutomation, defaultAutomation, migrateDepthRules, hasRules } from './computer-rules';
import type { AutomationProfile } from '../../shared/contracts/generated';
type WorkspaceEdit = Edit | { kind: 'automation'; value: AutomationProfile };
import { computed, ref, onScopeDispose } from 'vue';
import { ageFrame } from './telemetry';
import { invoke, isTauri } from '@tauri-apps/api/core';
import type {
  ApplyResult,
  ChangePreview,
  Edit,
  KeyboardSnapshot,
  MonitorFrame,
} from '../../shared/contracts/generated';
import { clone, profileEdits, projectEdits, readLocalProfile, type LocalProfile } from './model';

const emptyFrame = (): MonitorFrame => ({
  active: false,
  rulesEnabled: false,
  ruleFirings: 0,
  ruleError: null,
  travel: [],
  history: [],
  colors: [],
  colorAgeMs: null,
  packets: 0,
  message: null,
});
export function useWorkspace() {
  const native = isTauri();
  const snapshot = ref<KeyboardSnapshot | null>(null),
    edits = ref<WorkspaceEdit[]>([]),
    redo = ref<WorkspaceEdit[][]>([]),
    undoCounts = ref<number[]>([]);
  const busy = ref(''),
    error = ref(''),
    notice = ref(''),
    connected = ref(false),
    preview = ref<ChangePreview | null>(null);
  const appliedAutomation = ref<AutomationProfile>(defaultAutomation());
  try {
    const stored = localStorage.getItem('io.automation.v1');
    appliedAutomation.value = stored
      ? readAutomation(JSON.parse(stored))
      : migrateDepthRules(JSON.parse(localStorage.getItem('io.rules.v1') ?? '[]'));
  } catch {
    error.value = 'Не удалось прочитать программные действия.';
  }
  const automation = computed(() =>
    edits.value.reduce((p, e) => (e.kind === 'automation' ? e.value : p), appliedAutomation.value),
  );
  const automationDirty = computed(
    () => JSON.stringify(automation.value) !== JSON.stringify(appliedAutomation.value),
  );
  const hardwareEdits = computed(() =>
    edits.value.filter((e): e is Edit => e.kind !== 'automation'),
  );
  const startActions = ref(true);
  const ownershipPreview = ref<ChangePreview | null>(null);
  const actionsOnly = ref(false);
  const reviewAutomation = computed(() =>
    actionsOnly.value ? appliedAutomation.value : automation.value,
  );
  const live = ref<MonitorFrame>(emptyFrame());
  const receivedAt = ref(performance.now());
  const clock = ref(performance.now());
  const clockTimer = setInterval(() => {
    clock.value = performance.now();
  }, 50);
  onScopeDispose(() => clearInterval(clockTimer));
  const displayLive = computed(() => ageFrame(live.value, clock.value - receivedAt.value));
  const recoveryPreview = ref(false);
  const profiles = ref<LocalProfile[]>([]);
  try {
    const stored = JSON.parse(localStorage.getItem('io.profiles.v1') ?? '[]') as unknown[];
    profiles.value = stored.map((p) => readLocalProfile(JSON.stringify(p)));
  } catch {
    notice.value = 'Не удалось прочитать локальные профили. Можно импортировать резервный JSON.';
  }
  const draft = computed(() =>
    snapshot.value ? projectEdits(snapshot.value, hardwareEdits.value) : null,
  );
  function message(e: unknown) {
    return typeof e === 'object' && e && 'message' in e ? String(e.message) : String(e);
  }
  async function run<T>(label: string, operation: () => Promise<T>): Promise<T | undefined> {
    if (busy.value) return;
    busy.value = label;
    error.value = '';
    notice.value = '';
    try {
      return await operation();
    } catch (e) {
      error.value = message(e);
      if (
        typeof e === 'object' &&
        e &&
        'code' in e &&
        [
          'deviceIo',
          'deviceSelection',
          'timeout',
          'sessionLost',
          'shortWrite',
          'readbackMismatch',
          'invalidResponse',
        ].includes(String(e.code))
      ) {
        connected.value = false;
        live.value.active = false;
        live.value.rulesEnabled = false;
        preview.value = null;
      }
    } finally {
      busy.value = '';
    }
  }
  function stage(...values: WorkspaceEdit[]) {
    if (!snapshot.value || busy.value || !values.length) return;
    if (edits.value.length + values.length > 512) {
      error.value = 'Черновик достиг лимита. Примените изменения или отмените часть действий.';
      return;
    }
    for (const value of values)
      if (value.kind === 'automation') {
        try {
          readAutomation(value.value);
        } catch (e) {
          error.value = message(e);
          return;
        }
      }
    edits.value.push(...clone(values));
    undoCounts.value.push(values.length);
    redo.value = [];
    preview.value = null;
    notice.value = 'Изменение добавлено в черновик';
  }
  function undo() {
    if (busy.value) return;
    const count = undoCounts.value.pop();
    if (count) redo.value.push(edits.value.splice(-count));
    preview.value = null;
  }
  function redoEdit() {
    if (busy.value) return;
    const group = redo.value.pop();
    if (group) {
      edits.value.push(...group);
      undoCounts.value.push(group.length);
    }
    preview.value = null;
  }
  function discard() {
    edits.value = [];
    redo.value = [];
    undoCounts.value = [];
    preview.value = null;
    notice.value = 'Черновик отменён';
  }
  async function connect() {
    await run('Подключение…', async () => {
      const localAutomation = clone(automation.value);
      const local = !connected.value && draft.value ? clone(draft.value) : null;
      connected.value = false;
      const next = await invoke<KeyboardSnapshot>('connect_device');
      snapshot.value = next;
      connected.value = true;
      discard();
      live.value = emptyFrame();
      if (local) {
        edits.value = [
          ...profileEdits(next, local),
          { kind: 'automation', value: localAutomation },
        ];
        undoCounts.value = edits.value.length ? [edits.value.length] : [];
      }
      notice.value = local
        ? 'Конфигурация прочитана. Отличия локального профиля добавлены в черновик.'
        : 'Конфигурация прочитана с клавиатуры';
    });
  }
  async function disconnect() {
    await run('Отключение…', async () => {
      await invoke('disconnect_device');
      connected.value = false;
      live.value.active = false;
      live.value.rulesEnabled = false;
      notice.value = 'Снимок остаётся доступен для редактирования';
    });
  }
  async function refresh() {
    await run('Чтение…', async () => {
      const next = await invoke<KeyboardSnapshot>('refresh_device');
      snapshot.value = next;
      discard();
      live.value.active = false;
      live.value.rulesEnabled = false;
      notice.value = 'Состояние обновлено';
    });
  }
  async function monitor() {
    await run(live.value.active ? 'Остановка…' : 'Включение наблюдения…', async () => {
      live.value = await invoke<MonitorFrame>('set_monitor', { enabled: !live.value.active });
      receivedAt.value = performance.now();
    });
  }
  let polling = false;
  async function poll() {
    if (!native || !connected.value || polling) return;
    polling = true;
    try {
      live.value = await invoke<MonitorFrame>('monitor_frame');
      receivedAt.value = performance.now();
    } catch {
      live.value.active = false;
      live.value.rulesEnabled = false;
    } finally {
      polling = false;
    }
  }
  async function clearHistory() {
    await invoke('clear_history');
    live.value.history = [];
  }
  async function setHistoryCapacity(count: number) {
    live.value.history = live.value.history.slice(0, count);
    if (native) {
      try {
        await invoke('set_history_capacity', { count });
      } catch (e) {
        error.value = message(e);
      }
    }
  }
  async function prepare() {
    if (!snapshot.value) return;
    await run('Проверка изменений…', async () => {
      recoveryPreview.value = false;
      actionsOnly.value = false;
      ownershipPreview.value = null;
      startActions.value = automationDirty.value
        ? hasRules(automation.value)
        : live.value.rulesEnabled;
      readAutomation(automation.value);
      if (native) await invoke('validate_automation', { profile: clone(automation.value) });
      const hardware = hardwareEdits.value.length
        ? await invoke<ChangePreview>('prepare_changes', {
            request: { baseRevision: snapshot.value!.revision, edits: clone(hardwareEdits.value) },
          })
        : { token: '', changes: [] };
      if (native && connected.value && automation.value.depthChoices.length)
        ownershipPreview.value = await invoke<ChangePreview | null>('prepare_automation', {
          profile: clone(automation.value),
          baseRevision: hardware.token || snapshot.value!.revision,
        });
      preview.value = hardware;
    });
  }
  async function setActionsEnabled(enabled: boolean) {
    await run(enabled ? 'Включение действий…' : 'Остановка…', async () => {
      if (enabled && appliedAutomation.value.depthChoices.length) {
        if (!snapshot.value) return;
        actionsOnly.value = true;
        recoveryPreview.value = false;
        startActions.value = true;
        ownershipPreview.value = await invoke<ChangePreview | null>('prepare_automation', {
          profile: clone(appliedAutomation.value),
          baseRevision: snapshot.value.revision,
        });
        preview.value = { token: '', changes: [] };
        return;
      }
      await invoke('configure_automation', {
        profile: clone(appliedAutomation.value),
        enabled,
        ownershipToken: null,
      });
      await poll();
    });
  }
  async function apply() {
    if (!preview.value) return;
    const prepared = preview.value;
    const next = clone(reviewAutomation.value);
    const only = actionsOnly.value;
    const ownershipToken = ownershipPreview.value?.token ?? null;
    const isRecovery = recoveryPreview.value;
    preview.value = null;
    await run('Применение…', async () => {
      if (only) {
        await invoke('configure_automation', { profile: next, enabled: true, ownershipToken });
        await poll();
        actionsOnly.value = false;
        notice.value = 'Выбор по глубине включён. Отпустите клавиши перед первым нажатием.';
        return;
      }
      if (prepared.token && prepared.changes.length) {
        const result = await invoke<ApplyResult>('apply_changes', { token: prepared.token });
        snapshot.value = result.snapshot;
        edits.value = [{ kind: 'automation', value: next }];
        undoCounts.value = [1];
        redo.value = [];
        live.value.active = false;
        live.value.rulesEnabled = false;
        notice.value = 'Настройки клавиатуры применены и проверены.';
      }
      if (isRecovery) {
        edits.value = automationDirty.value ? [{ kind: 'automation', value: next }] : [];
        undoCounts.value = edits.value.length ? [1] : [];
        notice.value = 'Настройки клавиатуры восстановлены.';
        return;
      }
      const previous = localStorage.getItem('io.automation.v1');
      localStorage.setItem('io.automation.v1', JSON.stringify(next));
      try {
        if (native && connected.value) {
          const enabled = startActions.value && hasRules(next);
          await invoke('configure_automation', { profile: next, enabled, ownershipToken });
          await poll();
        }
      } catch (e) {
        if (previous === null) localStorage.removeItem('io.automation.v1');
        else localStorage.setItem('io.automation.v1', previous);
        throw e;
      }
      appliedAutomation.value = next;
      discard();
      notice.value =
        native && connected.value && startActions.value && hasRules(next)
          ? 'Применено. Жесты компьютера включены.'
          : 'Применено. Действия сохранены на компьютере.';
    });
  }
  async function recovery() {
    await run('Подготовка восстановления…', async () => {
      recoveryPreview.value = true;
      actionsOnly.value = false;
      ownershipPreview.value = null;
      preview.value = await invoke<ChangePreview>('prepare_recovery');
      notice.value =
        'Подготовлено восстановление затронутых блоков. Проверьте список перед применением.';
    });
  }
  function saveProfile(name: string) {
    if (!draft.value || !name.trim()) return;
    const profile: LocalProfile = {
      schemaVersion: 2,
      automation: clone(automation.value),
      name: name.trim().slice(0, 100),
      savedAt: new Date().toISOString(),
      snapshot: clone(draft.value),
    };
    try {
      const next = [profile, ...profiles.value.filter((p) => p.name !== profile.name)].slice(0, 40);
      localStorage.setItem('io.profiles.v1', JSON.stringify(next));
      profiles.value = next;
      notice.value = 'Профиль сохранён на этом компьютере';
    } catch {
      error.value = 'Недостаточно места для профиля. Экспортируйте JSON.';
    }
  }
  function deleteProfile(name: string) {
    const next = profiles.value.filter((p) => p.name !== name);
    try {
      localStorage.setItem('io.profiles.v1', JSON.stringify(next));
      profiles.value = next;
    } catch {
      error.value = 'Не удалось удалить профиль';
    }
  }
  function exportProfile(name: string) {
    if (!draft.value) return;
    const value: LocalProfile = {
      schemaVersion: 2,
      automation: clone(automation.value),
      name: name.trim() || 'Мой профиль',
      savedAt: new Date().toISOString(),
      snapshot: clone(draft.value),
    };
    const url = URL.createObjectURL(
      new Blob([JSON.stringify(value, null, 2)], { type: 'application/json' }),
    );
    const link = document.createElement('a');
    link.href = url;
    link.download = 'io-profile.json';
    link.click();
    URL.revokeObjectURL(url);
  }
  return {
    native,
    snapshot,
    draft,
    edits,
    redo,
    busy,
    error,
    notice,
    connected,
    preview,
    live,
    displayLive,
    profiles,
    recoveryPreview,
    automation,
    appliedAutomation,
    automationDirty,
    hardwareEdits,
    startActions,
    ownershipPreview,
    actionsOnly,
    reviewAutomation,
    setActionsEnabled,
    stage,
    undo,
    redoEdit,
    discard,
    connect,
    disconnect,
    refresh,
    monitor,
    poll,
    clearHistory,
    setHistoryCapacity,
    prepare,
    apply,
    recovery,
    saveProfile,
    deleteProfile,
    exportProfile,
  };
}
