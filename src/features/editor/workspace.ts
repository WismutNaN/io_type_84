import { computed, ref } from 'vue';
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
    edits = ref<Edit[]>([]),
    redo = ref<Edit[][]>([]),
    undoCounts = ref<number[]>([]);
  const busy = ref(''),
    error = ref(''),
    notice = ref(''),
    connected = ref(false),
    preview = ref<ChangePreview | null>(null);
  const live = ref<MonitorFrame>(emptyFrame());
  const recoveryPreview = ref(false);
  const profiles = ref<LocalProfile[]>([]);
  try {
    const stored = JSON.parse(localStorage.getItem('io.profiles.v1') ?? '[]') as unknown[];
    profiles.value = stored.map((p) => readLocalProfile(JSON.stringify(p)));
  } catch {
    notice.value = 'Не удалось прочитать локальные профили. Можно импортировать резервный JSON.';
  }
  const draft = computed(() => (snapshot.value ? projectEdits(snapshot.value, edits.value) : null));
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
        preview.value = null;
      }
    } finally {
      busy.value = '';
    }
  }
  function stage(...values: Edit[]) {
    if (!snapshot.value || busy.value || !values.length) return;
    if (edits.value.length + values.length > 512) {
      error.value = 'Черновик достиг лимита. Примените изменения или отмените часть действий.';
      return;
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
      const local = !connected.value && draft.value ? clone(draft.value) : null;
      connected.value = false;
      const next = await invoke<KeyboardSnapshot>('connect_device');
      snapshot.value = next;
      connected.value = true;
      discard();
      live.value = emptyFrame();
      if (local) {
        edits.value = profileEdits(next, local);
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
      notice.value = 'Снимок остаётся доступен для редактирования';
    });
  }
  async function refresh() {
    await run('Чтение…', async () => {
      const next = await invoke<KeyboardSnapshot>('refresh_device');
      snapshot.value = next;
      discard();
      live.value.active = false;
      notice.value = 'Состояние обновлено';
    });
  }
  async function monitor() {
    await run(live.value.active ? 'Остановка…' : 'Включение наблюдения…', async () => {
      live.value = await invoke<MonitorFrame>('set_monitor', { enabled: !live.value.active });
    });
  }
  let polling = false;
  async function poll() {
    if (!native || !connected.value || polling) return;
    polling = true;
    try {
      live.value = await invoke<MonitorFrame>('monitor_frame');
    } catch {
      live.value.active = false;
    } finally {
      polling = false;
    }
  }
  async function clearHistory() {
    await invoke('clear_history');
    live.value.history = [];
  }
  async function prepare() {
    if (!snapshot.value) return;
    await run('Проверка изменений…', async () => {
      recoveryPreview.value = false;
      preview.value = await invoke<ChangePreview>('prepare_changes', {
        request: { baseRevision: snapshot.value!.revision, edits: clone(edits.value) },
      });
    });
  }
  async function apply() {
    if (!preview.value) return;
    const token = preview.value.token;
    preview.value = null;
    await run('Запись и проверка…', async () => {
      const result = await invoke<ApplyResult>('apply_changes', { token });
      snapshot.value = result.snapshot;
      discard();
      live.value.active = false;
      notice.value = 'Применено · обратное чтение совпало';
    });
  }
  async function recovery() {
    await run('Подготовка восстановления…', async () => {
      recoveryPreview.value = true;
      preview.value = await invoke<ChangePreview>('prepare_recovery');
      notice.value =
        'Подготовлено восстановление затронутых блоков. Проверьте список перед применением.';
    });
  }
  function saveProfile(name: string) {
    if (!draft.value || !name.trim()) return;
    const profile: LocalProfile = {
      schemaVersion: 1,
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
      schemaVersion: 1,
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
    profiles,
    recoveryPreview,
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
    prepare,
    apply,
    recovery,
    saveProfile,
    deleteProfile,
    exportProfile,
  };
}
