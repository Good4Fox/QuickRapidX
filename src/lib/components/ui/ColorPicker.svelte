<script lang="ts">
	/*
	 * Выбор цвета.
	 *
	 * Системное поле `<input type="color">` открывает диалог Windows — со своим
	 * фоном, своим шрифтом и прямыми углами. Посреди тёмного окна с золотом он
	 * выглядел чужой деталью, ровно как раньше выглядел системный список.
	 *
	 * Здесь всё своё: квадрат насыщенности и яркости, полоса тона и поле для
	 * шестнадцатеричной записи.
	 */
	type Rgb = [number, number, number];

	type Props = {
		value: Rgb;
		onpick: (value: Rgb) => void;
		label: string;
	};

	let { value, onpick, label }: Props = $props();

	let open = $state(false);
	let root = $state<HTMLElement | null>(null);
	let area = $state<HTMLElement | null>(null);
	let bar = $state<HTMLElement | null>(null);

	/*
	 * Тон хранится отдельно от цвета намеренно.
	 *
	 * При переводе цвета в тон-насыщенность-яркость тон теряется на краях: у
	 * чёрного и у белого его нет вовсе. Если каждый раз вычислять его заново,
	 * то стоило довести ползунок до края — и цвет прыгал бы на красный, а
	 * квадрат перекрашивался сам собой.
	 */
	let hue = $state(0);
	let saturation = $state(0);
	let brightness = $state(0);

	/** Правка идёт изнутри — снаружи пришедшее значение не перечитываем. */
	let editing = false;

	$effect(() => {
		const incoming = value;

		if (editing) return;

		const [h, s, v] = toHsv(incoming);

		// Тон сохраняем прежний там, где его в цвете нет
		if (s > 0) hue = h;
		saturation = s;
		brightness = v;
	});

	$effect(() => {
		if (!open) return;

		const onDown = (event: PointerEvent) => {
			if (root && !root.contains(event.target as Node)) open = false;
		};

		const onKey = (event: KeyboardEvent) => {
			if (event.key === 'Escape') open = false;
		};

		document.addEventListener('pointerdown', onDown, true);
		document.addEventListener('keydown', onKey);

		return () => {
			document.removeEventListener('pointerdown', onDown, true);
			document.removeEventListener('keydown', onKey);
		};
	});

	function toHsv([r, g, b]: Rgb): [number, number, number] {
		const max = Math.max(r, g, b);
		const min = Math.min(r, g, b);
		const span = max - min;

		let h = 0;
		if (span !== 0) {
			if (max === r) h = ((g - b) / span) % 6;
			else if (max === g) h = (b - r) / span + 2;
			else h = (r - g) / span + 4;
		}

		h = Math.round(h * 60);
		if (h < 0) h += 360;

		return [h, max === 0 ? 0 : span / max, max / 255];
	}

	function toRgb(h: number, s: number, v: number): Rgb {
		const c = v * s;
		const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
		const m = v - c;

		const [r, g, b] =
			h < 60
				? [c, x, 0]
				: h < 120
					? [x, c, 0]
					: h < 180
						? [0, c, x]
						: h < 240
							? [0, x, c]
							: h < 300
								? [x, 0, c]
								: [c, 0, x];

		return [
			Math.round((r + m) * 255),
			Math.round((g + m) * 255),
			Math.round((b + m) * 255)
		];
	}

	function toHex(color: Rgb): string {
		return '#' + color.map((part) => part.toString(16).padStart(2, '0')).join('');
	}

	const current = $derived(toRgb(hue, saturation, brightness));
	const hueColor = $derived(`rgb(${toRgb(hue, 1, 1).join(', ')})`);

	function emit() {
		editing = true;
		onpick(toRgb(hue, saturation, brightness));
		// Отпускаем после кадра: значение возвращается сюда через свойство, и
		// перечитывать его тем же кадром значило бы затереть только что
		// выбранное округлением
		queueMicrotask(() => (editing = false));
	}

	/** Тянуть можно и за пределами поля — палец редко попадает точно. */
	function track(node: HTMLElement, onMove: (x: number, y: number) => void) {
		return (event: PointerEvent) => {
			node.setPointerCapture(event.pointerId);

			const apply = (source: PointerEvent) => {
				const box = node.getBoundingClientRect();
				const x = Math.min(1, Math.max(0, (source.clientX - box.left) / box.width));
				const y = Math.min(1, Math.max(0, (source.clientY - box.top) / box.height));
				onMove(x, y);
				emit();
			};

			apply(event);

			const onPointerMove = (source: PointerEvent) => apply(source);
			const onPointerUp = () => {
				node.removeEventListener('pointermove', onPointerMove);
				node.removeEventListener('pointerup', onPointerUp);
			};

			node.addEventListener('pointermove', onPointerMove);
			node.addEventListener('pointerup', onPointerUp);
		};
	}

	function onHex(event: Event) {
		const raw = (event.target as HTMLInputElement).value.trim().replace(/^#/, '');
		if (!/^[0-9a-fA-F]{6}$/.test(raw)) return;

		const rgb: Rgb = [
			parseInt(raw.slice(0, 2), 16),
			parseInt(raw.slice(2, 4), 16),
			parseInt(raw.slice(4, 6), 16)
		];

		const [h, s, v] = toHsv(rgb);
		if (s > 0) hue = h;
		saturation = s;
		brightness = v;
		emit();
	}
</script>

<div class="qrx_pick" class:qrx_pick_open={open} bind:this={root}>
	<button
		class="qrx_pick_head"
		style="background-color: rgb({current.join(', ')})"
		aria-haspopup="dialog"
		aria-expanded={open}
		aria-label={label}
		onclick={() => (open = !open)}
	></button>

	{#if open}
		<div class="qrx_pick_panel" role="dialog" aria-label={label}>
			<!-- Насыщенность по горизонтали, яркость по вертикали -->
			<div
				class="qrx_pick_area"
				style="--hue: {hueColor}"
				bind:this={area}
				role="slider"
				tabindex="0"
				aria-label={label}
				aria-valuenow={Math.round(saturation * 100)}
				onpointerdown={area
					? track(area, (x, y) => {
							saturation = x;
							brightness = 1 - y;
						})
					: undefined}
			>
				<span
					class="qrx_pick_dot"
					style="left: {saturation * 100}%; top: {(1 - brightness) * 100}%"
				></span>
			</div>

			<div
				class="qrx_pick_bar"
				bind:this={bar}
				role="slider"
				tabindex="0"
				aria-label={label}
				aria-valuenow={hue}
				onpointerdown={bar ? track(bar, (x) => (hue = x * 360)) : undefined}
			>
				<span class="qrx_pick_knob" style="left: {(hue / 360) * 100}%"></span>
			</div>

			<div class="qrx_pick_foot">
				<span class="qrx_pick_chip" style="background-color: rgb({current.join(', ')})"></span>
				<input class="qrx_pick_hex" type="text" value={toHex(current)} oninput={onHex} spellcheck="false" />
			</div>
		</div>
	{/if}
</div>
