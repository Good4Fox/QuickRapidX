<script lang="ts">
	/*
	 * Всплывающие окошки в нижнем правом углу.
	 *
	 * Без них уведомление видно, только если открыть панель, — то есть о
	 * неудаче пользователь узнаёт случайно и позже. Окошко показывается сразу
	 * и само уходит; запись при этом остаётся в истории.
	 */
	import { flip } from 'svelte/animate';
	import { fly } from 'svelte/transition';

	import LevelIcon from '$lib/components/Notifications/LevelIcon.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifications } from '$lib/state/notifications.svelte';
</script>

<div class="qrx_toasts" aria-live="polite">
	{#each notifications.toasts as note (note.id)}
		<div
			class="qrx_toast qrx_note_{note.level}"
			animate:flip={{ duration: 200 }}
			in:fly={{ x: 24, duration: 220 }}
			out:fly={{ x: 24, duration: 160 }}
		>
			<div class="qrx_note_icon"><LevelIcon level={note.level} /></div>

			<div class="qrx_note_body">
				<div class="qrx_note_title">{note.title}</div>
				{#if note.text}
					<div class="qrx_note_text">{note.text}</div>
				{/if}
			</div>

			<button
				class="qrx_note_dismiss"
				aria-label={i18n.t.note_dismiss}
				onclick={() => notifications.hideToast(note.id)}
			>
				<svg width="9" height="9" viewBox="0 0 10 10" stroke="currentColor" stroke-width="1.4">
					<path d="M1 1 L9 9 M9 1 L1 9" />
				</svg>
			</button>
		</div>
	{/each}
</div>
