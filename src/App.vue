<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import KeyboardPreview from './shared/keyboard-view/KeyboardPreview.vue';
import { connectShell, type ShellConnection } from './shared/contracts/desktop';

type Section =
  | 'overview'
  | 'keys'
  | 'actuation'
  | 'lighting'
  | 'panel'
  | 'macros'
  | 'rules'
  | 'profiles'
  | 'diagnostics';
const active = ref<Section>('overview');
const shell = ref<ShellConnection>({ kind: 'loading' });
const items: Array<{ id: Section; label: string; icon: string }> = [
  { id: 'overview', label: 'Обзор', icon: '◫' },
  { id: 'keys', label: 'Назначения', icon: '⌘' },
  { id: 'actuation', label: 'Ход и Rapid Trigger', icon: '↕' },
  { id: 'lighting', label: 'Подсветка клавиш', icon: '◉' },
  { id: 'panel', label: 'LED-панель', icon: '▥' },
  { id: 'macros', label: 'Макросы', icon: '≋' },
  { id: 'rules', label: 'Правила и слои', icon: '◇' },
  { id: 'profiles', label: 'Профили', icon: '▤' },
];
const currentTitle = computed(() =>
  active.value === 'diagnostics'
    ? 'О приложении'
    : (items.find((item) => item.id === active.value)?.label ?? 'Обзор'),
);
const shellLabel = computed(() => {
  switch (shell.value.kind) {
    case 'desktop':
      return 'Настольное приложение';
    case 'browser':
      return 'Предпросмотр в браузере';
    case 'error':
      return 'Ошибка связи с приложением';
    case 'loading':
      return 'Запуск приложения…';
  }
});
const descriptions: Partial<Record<Section, string>> = {
  keys: 'Назначения обычных клавиш и Fn будут редактироваться на общей схеме клавиатуры.',
  actuation:
    'Здесь появятся глубина срабатывания, Rapid Trigger и мёртвые зоны для выбранных клавиш.',
  lighting: 'Цвета, аппаратные эффекты и градиенты будут настраиваться отдельно от LED-панели.',
  macros: 'Здесь будут последовательности действий, которые выполняются в самой клавиатуре.',
  rules: 'Программные слои, двойные нажатия и условия будут работать при запущенном помощнике.',
  profiles: 'Локальные профили объединят настройки клавиатуры, подсветку и программные правила.',
};
async function refreshShell() {
  shell.value = { kind: 'loading' };
  shell.value = await connectShell();
}
onMounted(refreshShell);
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <span class="brand-mark">io</span>
        <div>TYPE 84<small>Рабочее пространство</small></div>
      </div>
      <div class="nav-label">КЛАВИАТУРА</div>
      <nav aria-label="Разделы приложения">
        <button
          v-for="item in items"
          :key="item.id"
          class="nav-item"
          :class="{ active: active === item.id }"
          :aria-current="active === item.id ? 'page' : undefined"
          @click="active = item.id"
        >
          <span class="nav-icon" aria-hidden="true">{{ item.icon }}</span
          >{{ item.label }}
          <span v-if="item.id === 'panel'" class="nav-dot" aria-hidden="true"></span>
        </button>
      </nav>
      <div class="sidebar-bottom">
        <div class="companion-status">
          <span class="tiny-dot"></span>
          <div>Помощник<small>Пока не реализован</small></div>
        </div>
        <button
          class="nav-item"
          :class="{ active: active === 'diagnostics' }"
          @click="active = 'diagnostics'"
        >
          <span class="nav-icon" aria-hidden="true">ⓘ</span>О приложении
        </button>
        <div class="sidebar-version">Открытый проект <span>0.1.0</span></div>
      </div>
    </aside>

    <div class="workspace">
      <header class="topbar">
        <div class="breadcrumb">
          IO Type 84 <span>/</span> <strong>{{ currentTitle }}</strong>
        </div>
        <span class="connection-pill"><span class="tiny-dot"></span>Без сеанса устройства</span>
      </header>
      <main>
        <div class="page-heading">
          <div>
            <p class="eyebrow">ВАША КЛАВИАТУРА. ВАШИ ПРАВИЛА.</p>
            <h1>{{ active === 'overview' ? 'IO Type 84 Magnetic' : currentTitle }}</h1>
            <p class="page-description">
              {{
                active === 'overview'
                  ? 'Единое место для настройки клавиш, света и повседневных действий.'
                  : active === 'panel'
                    ? 'Отдельная область света и индикации на клавиатуре.'
                    : active === 'diagnostics'
                      ? 'Состояние настольной оболочки и её компонентов.'
                      : descriptions[active]
              }}
            </p>
          </div>
          <span class="edition">ПРОТОТИП</span>
        </div>

        <div class="notice" role="status">
          <span class="notice-symbol" aria-hidden="true">i</span>
          <div>
            <strong>Каркас приложения готов к развитию</strong>
            <p>
              Подключение к клавиатуре ещё не реализовано. Сейчас доступны навигация и макет;
              настройки устройства не читаются и не изменяются.
            </p>
          </div>
        </div>

        <template v-if="active === 'overview'">
          <section class="surface keyboard-surface">
            <div class="surface-heading">
              <div>
                <h2>Ваша раскладка</h2>
                <p>IO Type 84 · Magnetic White</p>
              </div>
              <span class="tag">USB · 84 клавиши</span>
            </div>
            <KeyboardPreview />
          </section>
          <div class="feature-grid">
            <button class="feature-card" @click="active = 'keys'">
              <span class="feature-number">01</span
              ><span class="feature-title">Клавиши и ход <span>↗</span></span
              ><span class="feature-copy">Назначения, Fn и точность срабатывания.</span
              ><span class="feature-status">Следующие этапы</span>
            </button>
            <button class="feature-card panel-card" @click="active = 'panel'">
              <span class="feature-number">02</span
              ><span class="feature-title">Свет и LED-панель <span>↗</span></span
              ><span class="feature-copy">Подсветка клавиш и отдельная индикация.</span
              ><span class="feature-status">Панель добавлена в проект</span>
            </button>
            <button class="feature-card" @click="active = 'rules'">
              <span class="feature-number">03</span
              ><span class="feature-title">Больше действий <span>↗</span></span
              ><span class="feature-copy">Слои, условия и программный помощник.</span
              ><span class="feature-status">После конфигуратора</span>
            </button>
          </div>
        </template>

        <template v-else-if="active === 'panel'">
          <section class="surface panel-surface">
            <div class="surface-heading">
              <div>
                <h2>LED-панель</h2>
                <p>Самостоятельная часть настройки освещения</p>
              </div>
              <span class="tag">Исследование</span>
            </div>
            <div class="panel-illustration" aria-hidden="true">
              <div class="light-strip"></div>
              <div class="panel-line"></div>
            </div>
            <p class="illustration-caption">
              Условное изображение. Геометрия панели ещё уточняется.
            </p>
            <div class="panel-facts">
              <div>
                <h3>Переливы</h3>
                <p>Штатный режим панели, описанный владельцем клавиатуры.</p>
              </div>
              <div>
                <h3>Отклик на нажатие</h3>
                <p>
                  Штатная индикация нажатий отдельных клавиш. Формат данных и управления ещё
                  исследуется.
                </p>
              </div>
            </div>
          </section>
          <div class="footnote">
            Настройка панели появится после проверки её адресов и команд. Она будет отделена от
            RGB-подсветки клавиш.
          </div>
        </template>

        <section v-else-if="active === 'diagnostics'" class="surface diagnostics">
          <div class="surface-heading">
            <div>
              <h2>Состояние приложения</h2>
              <p aria-live="polite">{{ shellLabel }}</p>
            </div>
            <button
              class="secondary-button"
              :disabled="shell.kind === 'loading'"
              @click="refreshShell"
            >
              Проверить приложение
            </button>
          </div>
          <dl>
            <div>
              <dt>Режим</dt>
              <dd>{{ shellLabel }}</dd>
            </div>
            <div>
              <dt>Версия</dt>
              <dd>{{ shell.kind === 'desktop' ? shell.info.version : '0.1.0 · интерфейс' }}</dd>
            </div>
            <div>
              <dt>Платформа Rust</dt>
              <dd>
                {{ shell.kind === 'desktop' ? shell.info.platform : 'Недоступна в браузере' }}
              </dd>
            </div>
            <div>
              <dt>Связь Vue → Rust</dt>
              <dd>
                {{
                  shell.kind === 'desktop'
                    ? 'Ответ получен'
                    : shell.kind === 'loading'
                      ? 'Проверка…'
                      : 'Не установлена'
                }}
              </dd>
            </div>
            <div>
              <dt>Доступ к клавиатуре</dt>
              <dd>Транспорт ещё не реализован</dd>
            </div>
          </dl>
          <p v-if="shell.kind === 'error'" class="error-message" role="alert">
            {{ shell.message }}
          </p>
          <p class="footnote">
            Проверка обращается только к настольной оболочке. Она не устанавливает соединение с
            клавиатурой.
          </p>
        </section>

        <section v-else class="surface planned-surface">
          <span class="planned-icon" aria-hidden="true">{{
            items.find((item) => item.id === active)?.icon
          }}</span>
          <h2>{{ currentTitle }}</h2>
          <p>{{ descriptions[active] }}</p>
          <span class="tag">Редактор появится на следующем этапе</span
          ><button class="secondary-button" @click="active = 'overview'">Вернуться к обзору</button>
        </section>

        <footer>
          <span class="footer-runtime"
            ><span class="tiny-dot" :class="{ ready: shell.kind === 'desktop' }"></span
            >{{ shellLabel }}</span
          ><span>Windows + USB · локальные настройки</span>
        </footer>
      </main>
    </div>
  </div>
</template>
