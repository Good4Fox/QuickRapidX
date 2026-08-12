<script lang="ts">
	/*
	 * Значок с двумя действиями: нажатие и удержание.
	 *
	 * Заведён ради крестика. Один крестик не может значить сразу и «спрятать», и
	 * «выйти», а нужны оба: программа живёт в трее, и закрывать её насовсем
	 * приходится редко — но приходится.
	 *
	 * Развести их отдельными кнопками нельзя: две одинаковые крестообразные
	 * кнопки рядом человек различать не станет и рано или поздно промахнётся
	 * туда, где выход. Развести временем — можно: промахнуться легко, а
	 * случайно продержать кнопку секунду нельзя. Тот же приём и по той же
	 * причине уже стоит на «запустить весь набор» — там `HoldButton`.
	 *
	 * Здесь своя, а не тот же `HoldButton`: у него подпись, пояснение и высота
	 * в две строки — в полосе заголовка такому места нет.
	 */
	type Props = {
		/** Класс значка — маска из icons.css. */
		ico: string;
		/** Класс самой кнопки: оформление у полос заголовка разное. */
		button: string;
		/** Что делает короткое нажатие. */
		label: string;
		/** Что случится, если додержать. */
		holdLabel: string;
		onclick: () => void;
		onhold: () => void;
		/** Сколько держать, миллисекунды. */
		hold?: number;
	};

	let { ico, button, label, holdLabel, onclick, onhold, hold = 900 }: Props = $props();

	/** 0–1: сколько набралось. Кольцо вокруг значка рисуется по нему. */
	let progress = $state(0);

	/** Удержание сработало — отпускание не должно ещё и нажать. */
	let fired = $state(false);

	let frame = 0;

	function begin(event: PointerEvent) {
		// Левая кнопка и только она: правой вызывают меню окна
		if (event.button !== 0) return;

		// Захват указателя: без него уход курсора за пределы кнопки перестаёт
		// присылать события, и набранное зависло бы на месте
		(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);

		fired = false;

		const started = performance.now();

		const step = () => {
			progress = Math.min(1, (performance.now() - started) / hold);

			if (progress < 1) {
				frame = requestAnimationFrame(step);
				return;
			}

			fired = true;
			onhold();
		};

		frame = requestAnimationFrame(step);
	}

	function end(event: PointerEvent) {
		cancelAnimationFrame(frame);

		const held = fired;

		progress = 0;
		fired = false;

		if (held) return;

		/*
		 * Отпускание за пределами кнопки не считается нажатием.
		 *
		 * Захват указателя присылает нам отпускание, где бы ни был курсор, — и
		 * без этой проверки увести палец в сторону, чтобы передумать, было бы
		 * нельзя. Для крестика это важнее обычного: передумывать тут есть о чём.
		 */
		const box = (event.currentTarget as HTMLElement).getBoundingClientRect();

		const inside =
			event.clientX >= box.left &&
			event.clientX <= box.right &&
			event.clientY >= box.top &&
			event.clientY <= box.bottom;

		if (inside) onclick();
	}

	function cancel() {
		cancelAnimationFrame(frame);
		progress = 0;
		fired = false;
	}
</script>

<!--
	Подсказка меняется под пальцем: пока держат, она говорит, что будет, если
	не отпускать. Без этого удержание оставалось бы тайным знанием.
-->
<button
	class={button}
	class:qrx_hold_ico_armed={progress > 0}
	style="--hold: {progress}"
	aria-label={label}
	data-tooltip={progress > 0 ? holdLabel : `${label} · ${holdLabel}`}
	data-tooltip-position="bottom"
	onpointerdown={begin}
	onpointerup={end}
	onpointercancel={cancel}
	onkeydown={(event) => {
		// С клавиатуры удержания нет — Enter даёт короткое нажатие
		if (event.key !== 'Enter' && event.key !== ' ') return;
		event.preventDefault();
		onclick();
	}}
>
	<div class={ico}></div>
</button>
