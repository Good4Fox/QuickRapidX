import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
// @ts-expect-error type error without @types/node package
import { realpathSync } from "node:fs";
const host = process.env.TAURI_DEV_HOST;

// Папка проекта открывается через junction (C:\Desktop\… → F:\Dev\…). Модули
// Rolldown резолвит по настоящему пути, а имена чанков SvelteKit считает
// относительно рабочей папки. Если та осталась ссылкой, стороны оказываются на
// разных дисках: относительный путь между ними не строится, имена вырождаются в
// «F_/Dev/…» и сборка падает. Разворачиваем ссылку до того, как это посчитают.
process.chdir(realpathSync(process.cwd()));

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [sveltekit()],

  build: {
    /*
     * Значки — отдельными файлами, а не строками внутри стилей.
     *
     * По умолчанию Vite вшивает мелкие файлы прямо в CSS ссылкой `data:`. У нас
     * значки нарисованы масками, и один и тот же файл упоминается из десятка
     * правил — вшиваясь в каждое заново. Замерено на собранном листе стилей:
     * 158 таких ссылок, но лишь 69 разных, то есть 127 КБ из 359 — один и тот
     * же текст, повторённый по кругу. И разбирают его все три окна.
     *
     * Ноль отключает вшивание совсем: каждый значок становится файлом, который
     * берут один раз. Обращений больше, но идут они к своему же протоколу —
     * это дешевле, чем разбирать лишние сто с лишним килобайт в каждом окне.
     */
    assetsInlineLimit: 0,
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
