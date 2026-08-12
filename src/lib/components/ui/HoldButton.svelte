<script lang="ts">
	/*
	 * Кнопка, срабатывающая от удержания.
	 *
	 * Защита от случайного нажатия для действий, которые нельзя отменить:
	 * выключение, перезагрузка, блокировка, сон.
	 *
	 * Прежде на этом месте стоял отдельный флажок «подтверждаю»: он висел рядом
	 * с заголовком, ничего не объяснял, гасил все пять кнопок разом — включая
	 * безобидное «открыть параметры» — и сбрасывался после каждого действия.
	 * Причём защищал он плохо: отметить флажок и промахнуться мышью по соседней
	 * кнопке было ровно так же легко.
	 *
	 * Удержание защищает самим действием: промахнуться можно, но случайно
	 * продержать кнопку почти секунду — нет. И подсказка не нужна: полоса,
	 * которая набирается под пальцем, объясняет себя сама.
	 */
	type Props = {
		label: string;
		/** Класс значка — маска из icons.css. */
		ico: string;
		/** Подпись «удерживайте», пока кнопка не нажата. */
		hint: string;
		onrun: () => void;
		/** Сколько держать, миллисекунды. */
		hold?: number;
		/**
		 * Уменьшенный вид — для рядов с обычными кнопками.
		 *
		 * Полный вид рассчитан на отдельно стоящее действие вроде выключения
		 * компьютера: крупный значок, подпись и пояснение под ней. В ряду с
		 * мелкими кнопками он выглядит чужим — вдвое выше соседей.
		 */
		compact?: boolean;
	};

	let { label, ico, hint, onrun, hold = 900, compact = false }: Props = $props();

	let progress = $state(0);
	let done = $state(false);
	let frame = 0;
	let keyHeld = false;

	function begin() {
		const started = performance.now();

		const tick = (now: number) => {
			progress = Math.min(1, (now - started) / hold);

			if (progress >= 1) {
				fire();
				return;
			}

			frame = requestAnimationFrame(tick);
		};

		cancelAnimationFrame(frame);
		frame = requestAnimationFrame(tick);
	}

	function cancel() {
		cancelAnimationFrame(frame);
		progress = 0;
	}

	function fire() {
		cancelAnimationFrame(frame);
		progress = 0;

		// Отметка держится недолго: подтверждение, что нажатие засчитано —
		// само действие может увести систему в сон и вернуться нескоро
		done = true;
		setTimeout(() => (done = false), 1200);

		onrun();
	}

	// Пробел и Enter удерживаются так же. onkeydown повторяется, пока клавиша
	// зажата, — считаем только первое срабатывание.
	function onKeyDown(event: KeyboardEvent) {
		if (event.key !== ' ' && event.key !== 'Enter') return;
		event.preventDefault();
		if (keyHeld) return;
		keyHeld = true;
		begin();
	}

	function onKeyUp() {
		keyHeld = false;
		cancel();
	}

	$effect(() => () => cancelAnimationFrame(frame));
</script>

<button
	class="qrx_hold"
	class:qrx_hold_small={compact}
	class:qrx_hold_armed={progress > 0}
	class:qrx_hold_done={done}
	style="--hold: {progress}"
	data-tooltip={compact ? hint : undefined}
	data-tooltip-position={compact ? 'top' : undefined}
	onpointerdown={begin}
	onpointerup={cancel}
	onpointerleave={cancel}
	onpointercancel={cancel}
	onkeydown={onKeyDown}
	onkeyup={onKeyUp}
>
	<span class="qrx_hold_ico {ico}" aria-hidden="true"></span>
	<span class="qrx_hold_label">{label}</span>
	<span class="qrx_hold_hint">{hint}</span>
	<span class="qrx_hold_bar" aria-hidden="true"></span>
</button>
