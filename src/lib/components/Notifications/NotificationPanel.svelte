<script lang="ts">
	/*
	 * Панель уведомлений, выпадающая из полосы заголовка.
	 *
	 * Прошлая версия показывала список карточек без заголовка, времени и
	 * возможности убрать одну запись — только «очистить всё». И наполнять её
	 * было нечем: уведомления никто не создавал.
	 */
	import { fly } from 'svelte/transition';

	import { clickOutside } from '$lib/actions/clickOutside';
	import LevelIcon from '$lib/components/Notifications/LevelIcon.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { notifications, relativeTime } from '$lib/state/notifications.svelte';

	type Props = { onclose: () => void };

	let { onclose }: Props = $props();

	/*
	 * Пересчитываем «5 мин назад» раз в полминуты: без этого свежая запись
	 * так и осталась бы «только что», пока панель открыта.
	 */
	let now = $state(Date.now());

	$effect(() => {
		const timer = setInterval(() => {
			now = Date.now();
		}, 30_000);

		return () => clearInterval(timer);
	});

	// Открыли панель — значит, увидели. Помечаем прочитанным на выходе,
	// чтобы точка непрочитанного не исчезала под курсором.
	$effect(() => {
		return () => notifications.markAllRead();
	});

	/*
	 * Очистка в два нажатия: первое взводит, второе выполняет. Кнопка стоит
	 * рядом с крестиком, и промахнуться по ней слишком легко, а история
	 * уведомлений после очистки не восстанавливается.
	 */
	let armed = $state(false);
	let disarm: ReturnType<typeof setTimeout> | undefined;

	$effect(() => () => clearTimeout(disarm));

	function clearAll() {
		if (!armed) {
			armed = true;
			clearTimeout(disarm);
			disarm = setTimeout(() => (armed = false), 3000);
			return;
		}

		clearTimeout(disarm);
		armed = false;
		notifications.clear();
	}
</script>

<div
	class="qrx_notes"
	use:clickOutside={onclose}
	transition:fly={{ y: -8, duration: 180 }}
>
	<header class="qrx_notes_head">
		<span class="qrx_notes_title">{i18n.t.notification_text_1}</span>
		{#if notifications.unread > 0}
			<span class="qrx_notes_count">{notifications.unread}</span>
		{/if}

		<div class="qrx_notes_tools">
			{#if notifications.any}
				<button
					class="qrx_notes_tool qrx_notes_clear"
					class:qrx_notes_clear_armed={armed}
					aria-label={i18n.t.notification_text_2}
					data-tooltip={armed ? i18n.t.note_clear_confirm : i18n.t.notification_text_2}
					data-tooltip-position="bottom"
					onclick={clearAll}
				>
					<svg
						width="15"
						height="15"
						viewBox="0 0 24 24"
						fill="none"
						stroke="currentColor"
						stroke-width="1.7"
						stroke-linecap="round"
						stroke-linejoin="round"
					>
						<!-- Крышка отделена: на наведении она приподнимается -->
						<g class="qrx_trash_lid">
							<path d="M3.5 6.5h17" />
							<path d="M9.5 6.5V4.6a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v1.9" />
						</g>
						<path d="M5.8 6.5l.9 13.1a1 1 0 0 0 1 .9h8.6a1 1 0 0 0 1-.9l.9-13.1" />
						<path d="M10.2 10.4v6M13.8 10.4v6" />
					</svg>
				</button>
			{/if}

			<button
				class="qrx_notes_tool"
				aria-label={i18n.t.grp_close}
				data-tooltip={i18n.t.grp_close}
				data-tooltip-position="bottom"
				onclick={onclose}
			>
				<svg width="10" height="10" viewBox="0 0 10 10" stroke="currentColor" stroke-width="1.4">
					<path d="M1 1 L9 9 M9 1 L1 9" />
				</svg>
			</button>
		</div>
	</header>

	<div class="qrx_notes_list">
		{#each notifications.items as note (note.id)}
			<article class="qrx_note qrx_note_{note.level}" class:qrx_note_unread={!note.read}>
				<div class="qrx_note_icon"><LevelIcon level={note.level} /></div>

				<div class="qrx_note_body">
					<div class="qrx_note_title">{note.title}</div>
					{#if note.text}
						<div class="qrx_note_text">{note.text}</div>
					{/if}
					<div class="qrx_note_time">{relativeTime(note.at, now)}</div>
				</div>

				<button
					class="qrx_note_dismiss"
					aria-label={i18n.t.note_dismiss}
					onclick={() => notifications.dismiss(note.id)}
				>
					<svg width="9" height="9" viewBox="0 0 10 10" stroke="currentColor" stroke-width="1.4">
						<path d="M1 1 L9 9 M9 1 L1 9" />
					</svg>
				</button>
			</article>
		{:else}
			<div class="qrx_notes_empty">
				<LevelIcon level="info" size={30} />
				<span>{i18n.t.grp_items_none}</span>
			</div>
		{/each}
	</div>
</div>
