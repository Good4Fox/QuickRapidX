<script lang="ts">
	/*
	 * Раздел «О программе».
	 *
	 * Собран из тех же карточек, что и остальные настройки. Прежняя версия была
	 * одной тёмной плитой с двумя половинами: при нажатии «подробнее» левая
	 * половина пропадала через display: none, а правая раздувалась до 350%
	 * ширины с отступом в 15vh, и аватар менял размер с 14vw на 8vw. Здесь
	 * карточка просто становится выше.
	 */
	import { openUrl } from '@tauri-apps/plugin-opener';

	import { APP_TAG, APP_TAG_TOOLTIP } from '$lib/data/info';
	import { i18n } from '$lib/state/i18n.svelte';

	type Props = { versions: string };

	// Значение приходит из оболочки настроек, но здесь удобнее собрать метки
	// самим — на них подпись и версия стоят раздельно.
	let { versions }: Props = $props();

	/** Развёрнутый рассказ об авторе. */
	let expanded = $state(false);

	/*
	 * Скрытая строка — открывается щелчком по знаку приложения.
	 *
	 * В исходном приложении её открывала невидимая полоса пустого места под
	 * карточкой: <div> во всю ширину с символом-заполнителем вместо подписи.
	 * Знак заметнее и не ловит случайные щелчки мимо, а сама строка осталась
	 * оранжевой — этим цветом она и отличалась от всего остального.
	 */
	let secret = $state(false);

	/** Строка состояния ядра сменяется сама. */
	let stateIndex = $state(0);

	const CORE_STATES = $derived([
		i18n.t.about_core_state_1,
		i18n.t.about_core_state_2,
		i18n.t.about_core_state_3,
		i18n.t.about_core_state_4
	]);

	$effect(() => {
		const timer = setInterval(() => {
			stateIndex = (stateIndex + 1) % 4;
		}, 2000);

		return () => clearInterval(timer);
	});

	const LINKS = {
		releases: 'https://github.com/Good4Fox/QuickRapidX/releases',
		author: 'https://github.com/Good4Fox',
		repo: 'https://github.com/Good4Fox/QuickRapidX',
		qa: 'https://github.com/Good4Fox/QuickRapidX/discussions/categories/q-a',
		discussions: 'https://github.com/Good4Fox/QuickRapidX/discussions'
	};

	const HELP = $derived([
		{
			ico: 1,
			url: LINKS.repo,
			title: i18n.t.settings_text_10_10,
			note: i18n.t.settings_text_10_11
		},
		{ ico: 2, url: LINKS.qa, title: i18n.t.settings_text_10_12, note: i18n.t.settings_text_10_13 },
		{
			ico: 3,
			url: LINKS.discussions,
			title: i18n.t.settings_text_10_14,
			note: i18n.t.settings_text_10_15
		}
	]);
</script>

<div class="qrx_about">
	<!-- Знак и название -->
	<section class="qrx_about_card qrx_about_hero">
		<button
			class="qrx_about_mark"
			aria-label={i18n.t.home_app_text_1}
			onclick={() => (secret = !secret)}
		>
			<div class="qrx_about_mark_ico"></div>
		</button>

		<div class="qrx_about_id">
			<span class="qrx_about_name">{i18n.t.home_app_text_1}</span>
			<span class="qrx_about_role">{i18n.t.settings_text_10_1}</span>

			<div class="qrx_about_tags">
				<span class="qrx_about_tag">v{APP_TAG_TOOLTIP}</span>
				<span class="qrx_about_tag">{i18n.t.app_stage || APP_TAG}</span>
				<!-- Исходники открыты — метка ведёт в репозиторий -->
				<button class="qrx_about_tag qrx_about_tag_link" onclick={() => openUrl(LINKS.repo)}>
					{i18n.t.settings_text_10_0} · MIT
				</button>
			</div>
		</div>

		<button class="qrx_about_button" onclick={() => openUrl(LINKS.releases)}>
			{i18n.t.settings_text_10_3}
		</button>
	</section>

	{#if secret}
		<div class="qrx_about_secret">{i18n.t.appSecretText_1}</div>
	{/if}

	<!-- Автор -->
	<section class="qrx_about_card qrx_about_author">
		<div class="qrx_about_author_head">
			<div class="qrx_about_avatar"></div>

			<div class="qrx_about_id">
				<span class="qrx_about_name">{i18n.t.home_app_text_2}</span>
				<span class="qrx_about_role">{i18n.t.settings_text_10_7}</span>
			</div>

			<button class="qrx_about_button" onclick={() => openUrl(LINKS.author)}>
				{i18n.t.settings_text_10_6}
			</button>
		</div>

		<p class="qrx_about_bio">
			{expanded ? i18n.t.settings_text_10_8_2 : i18n.t.settings_text_10_8}
		</p>

		<button class="qrx_about_more" onclick={() => (expanded = !expanded)}>
			{expanded ? i18n.t.settings_text_10_9_2 : i18n.t.settings_text_10_9}
		</button>
	</section>

	<!-- Когнитивное ядро — то, над чем автор работает сейчас -->
	<section class="qrx_about_card qrx_about_core">
		<div class="qrx_about_core_head">
			<div class="qrx_about_core_mark" aria-hidden="true">
				<span class="qrx_about_core_ring"></span>
				<span class="qrx_about_core_ring_in"></span>
				<span class="qrx_about_core_dot"></span>
			</div>

			<div class="qrx_about_id">
				<span class="qrx_about_name">{i18n.t.about_core_title}</span>
				<span class="qrx_about_core_state">{CORE_STATES[stateIndex]}</span>
			</div>

			<span class="qrx_about_tag">{i18n.t.about_core_wip}</span>
		</div>

		<p class="qrx_about_bio">{i18n.t.about_core_text}</p>
	</section>

	<!-- Помощь -->
	<div class="qrx_about_links">
		{#each HELP as link (link.url)}
			<button class="qrx_about_link" onclick={() => openUrl(link.url)}>
				<div class="qrx_about_link_tile">
					<div class="qrx_about_link_ico qrx_about_link_ico_{link.ico}"></div>
				</div>

				<div class="qrx_about_link_text">
					<span class="qrx_about_link_title">{link.title}</span>
					<span class="qrx_about_link_note">{link.note}</span>
				</div>

				<span class="qrx_about_link_go" aria-hidden="true">&rarr;</span>
			</button>
		{/each}
	</div>

	<footer class="qrx_about_foot">
		<span>{i18n.t.settings_text_10_4}</span>
		<span>{i18n.t.settings_text_10_5}</span>
		<span>{i18n.t.settings_text_10_2}{versions}</span>
	</footer>
</div>
