<script lang="ts">
	/*
	 * Переход между экранами.
	 *
	 * Уходящий и приходящий экраны существуют одновременно, поэтому оба
	 * выкладываются поверх контейнера (`position: absolute`) — иначе на время
	 * перехода в потоке оказывались бы два блока и раскладка прыгала бы.
	 *
	 * Двигаются только `opacity` и `transform`: их браузер считает на
	 * композиторе, без пересчёта раскладки на каждом кадре.
	 */
	import type { Snippet } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import { fly } from 'svelte/transition';

	type Props = {
		/** Ключ экрана: его смена и запускает переход. */
		view: string;
		/**
		 * Куда движемся: `1` — вперёд, `-1` — назад. Приходящий экран
		 * выезжает с той стороны, в которую идёт переход.
		 */
		direction?: number;
		/** Ось движения. Разделы едут вбок, настройки — снизу. */
		axis?: 'x' | 'y';
		/** Насколько сдвигать, пиксели. */
		shift?: number;
		children: Snippet;
	};

	let { view, direction = 1, axis = 'x', shift = 26, children }: Props = $props();

	/** Системная настройка «меньше движения» — уважаем её. */
	const still =
		typeof window !== 'undefined' &&
		window.matchMedia('(prefers-reduced-motion: reduce)').matches;

	const offset = $derived(shift * direction);

	function enter(node: Element) {
		return fly(node, {
			[axis]: offset,
			duration: still ? 0 : 260,
			delay: still ? 0 : 90,
			easing: cubicOut
		});
	}

	function leave(node: Element) {
		return fly(node, {
			[axis]: -offset,
			duration: still ? 0 : 160,
			easing: cubicOut
		});
	}
</script>

<div class="qrx_switch">
	{#key view}
		<div class="qrx_switch_item" in:enter out:leave>
			{@render children()}
		</div>
	{/key}
</div>
