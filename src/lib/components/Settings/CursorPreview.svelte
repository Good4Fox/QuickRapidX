<script lang="ts">
	/*
	 * Курсор пользователя в предпросмотре.
	 *
	 * Ядро отдаёт кадры сырыми пикселями, а не картинкой: кодировать PNG в
	 * ядре значило бы тянуть ради этого целую библиотеку, а здесь всё равно
	 * рисовать на холсте.
	 *
	 * Кадров может быть много — курсор бывает и с анимацией. Если кадр один,
	 * ничего не крутится: таймер не заводится вовсе.
	 *
	 * Не получилось — не беда: показываем прежнюю нарисованную стрелку. Курсор
	 * может не прочитаться, если схема ссылается на файл, которого уже нет.
	 */
	import { invoke } from '@tauri-apps/api/core';

	type CursorImage = {
		width: number;
		height: number;
		hot_x: number;
		hot_y: number;
		frames: number[][];
		delay: number;
	};

	let canvas = $state<HTMLCanvasElement | null>(null);
	let failed = $state(false);

	/*
	 * Точка привязки в точках картинки.
	 *
	 * Ею курсор «указывает»: у стрелки это кончик, у руки — палец. Выделение
	 * тянется именно от неё, поэтому картинку надо сдвинуть так, чтобы эта
	 * точка легла в угол рамки, а не левый верхний угол изображения.
	 */
	let hot = $state({ x: 0, y: 0 });

	$effect(() => {
		const target = canvas;
		if (!target) return;

		let stop = false;
		let timer = 0;

		(async () => {
			let image: CursorImage;

			try {
				image = await invoke<CursorImage>('cursor_default');
			} catch (e) {
				console.error('не удалось получить курсор:', e);
				failed = true;
				return;
			}

			if (stop || image.frames.length === 0) return;

			target.width = image.width;
			target.height = image.height;
			hot = { x: image.hot_x, y: image.hot_y };

			const context = target.getContext('2d');
			if (!context) return;

			// Кадры готовятся заранее: собирать ImageData на каждом шаге анимации
			// значило бы гонять сотни килобайт по десять раз в секунду
			const frames = image.frames.map((frame) => {
				const data = new ImageData(image.width, image.height);
				data.data.set(Uint8ClampedArray.from(frame));
				return data;
			});

			let index = 0;

			const draw = () => {
				if (stop) return;

				context.putImageData(frames[index], 0, 0);

				if (frames.length < 2) return;

				index = (index + 1) % frames.length;
				timer = window.setTimeout(draw, image.delay || 100);
			};

			draw();
		})();

		return () => {
			stop = true;
			clearTimeout(timer);
		};
	});
</script>

{#if failed}
	<!-- Запасной вариант: нарисованная стрелка, как было раньше -->
	<div class="qrx_win_box_cursor"></div>
{:else}
	<canvas
		class="qrx_cursor_shot"
		style="--hot-x: {hot.x}px; --hot-y: {hot.y}px"
		bind:this={canvas}
	></canvas>
{/if}
