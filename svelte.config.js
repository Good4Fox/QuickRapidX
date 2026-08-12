import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

// У Tauri нет Node-сервера, поэтому маршруты пререндерятся в статические HTML:
// каждое окно (/, /splash, /tray) грузит собственный файл.
// См. https://v2.tauri.app/start/frontend/sveltekit/

/** @type {import('@sveltejs/kit').Config} */
const config = {
	preprocess: vitePreprocess(),
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			precompress: false,
			strict: true
		})
	}
};

export default config;
