import { createApp } from 'vue';
import App from './App.vue';
import './style.css';
import { initPreferences } from './shared/ui/preferences';
initPreferences();

createApp(App).mount('#app');
