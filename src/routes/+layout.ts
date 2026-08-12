// У Tauri нет Node-сервера, поэтому SSR выключен, а маршруты пререндерятся
// в статические HTML — каждое окно грузит свой файл напрямую.
// См. https://v2.tauri.app/start/frontend/sveltekit/
export const ssr = false;
export const prerender = true;

// Каталогами, а не файлами: сборка даёт splash/index.html вместо splash.html,
// поэтому адрес окна (/splash/) совпадает в dev-сервере и в собранном бандле.
// Резолвер Tauri принял бы оба варианта — он пробует path, path.html,
// path/index.html и только потом index.html (tauri/src/manager/mod.rs,
// get_asset) — но одинаковый путь в обоих режимах экономит класс багов,
// на котором горела старая сборка.
export const trailingSlash = 'always';
