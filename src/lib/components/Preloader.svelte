<script lang="ts">
	/*
	 * Экран загрузки.
	 *
	 * Строится вокруг молнии из фирменного знака: она стоит в кольце, по
	 * которому идёт настоящий прогресс из ядра. По краям — стрелки, снизу —
	 * знак с названием.
	 *
	 * Прежняя анимация была отдельной фигурой из шести квадратов, которые
	 * перебирали clip-path по кругу: она ничего не сообщала о ходе загрузки и
	 * жила своей жизнью. Здесь движение привязано к делу — кольцо заполняется
	 * ровно настолько, насколько ядро отработало.
	 */
	import { boot, bootLabel } from '$lib/state/boot.svelte';
	import { i18n } from '$lib/state/i18n.svelte';

	/** Длина окружности кольца: r = 46, значит 2πr. */
	const RING = 2 * Math.PI * 46;

	const offset = $derived(RING - (RING * Math.min(100, boot.percent)) / 100);
</script>

<div data-tauri-drag-region class="qrx_load">
	<!-- Стрелки по краям -->
	<div class="qrx_load_arrows" aria-hidden="true">
		<div class="qrx_load_arrow qrx_load_arrow_left"></div>
		<div class="qrx_load_arrow qrx_load_arrow_right"></div>
	</div>

	<!-- Молния в кольце -->
	<div class="qrx_load_core">
		<svg class="qrx_load_ring" viewBox="0 0 100 100" aria-hidden="true">
			<circle class="qrx_load_ring_track" cx="50" cy="50" r="46" />
			<circle
				class="qrx_load_ring_value"
				cx="50"
				cy="50"
				r="46"
				stroke-dasharray={RING}
				stroke-dashoffset={offset}
			/>
		</svg>

		<svg class="qrx_load_bolt" viewBox="0 0 512 512" aria-hidden="true">
			<path
				d="M352.78 215.916C350.059 221.685 261.5 395.5 257.5 403C253.5 410.5 253.5 413 248 413C242.5 413 242.5 408 242.5 401.5C242.5 396.469 248.632 294.913 251.405 249.239C252.046 238.685 241.875 230.837 231.816 234.094C207.965 241.815 171.824 253.502 168.5 254.5C163.5 256 159.473 254.646 158 251.5C156.527 248.353 157.5 244.5 159.539 241.658C161.49 238.94 250.041 115.086 257.74 104.319C258.136 103.765 258.419 103.261 258.788 102.69C261.011 99.2508 264.273 98.5288 267 99.4996C269.945 100.549 271.528 104.622 271.528 108.818L268.531 200.314C268.209 210.146 277.266 217.626 286.857 215.44C308.338 210.544 340.851 203.163 344.5 202.5C350 201.5 354 201 355.5 205.5C357 210 355.5 210.148 352.78 215.916Z"
			/>
		</svg>
	</div>

	<!-- Шаг загрузки -->
	<div class="qrx_load_status">
		{#if boot.error}
			<span class="qrx_load_failed">{boot.error.message}</span>
		{:else}
			<span class="qrx_load_step">{bootLabel(i18n.t, boot.key)}</span>
		{/if}
	</div>

	<!-- Знак с названием -->
	<div class="qrx_load_brand">
		<div class="qrx_load_brand_ico"></div>
		<div class="qrx_load_brand_text">QuickRapidX</div>
	</div>
</div>
