<script lang="ts">
	/*
	 * Основные настройки: язык, тема, ссылки и поведение окна.
	 *
	 * Раскладка общая для всех разделов: список слева с направляющей, строки
	 * справа. Каждая строка — карточка с рамкой и матовой подложкой, как поле
	 * поиска на вкладке «Приложения». Прежде строки были рамкой без подложки
	 * внутри рамки списка внутри рамки экрана.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';

	import BinTray from '$lib/components/Settings/BinTray.svelte';
	import SettingsNav from '$lib/components/Settings/SettingsNav.svelte';
	import Select from '$lib/components/ui/Select.svelte';
	import { ICON_ORDER, ICON_PREVIEWS, appIcon } from '$lib/state/appIcon.svelte';
	import { LANGUAGES, i18n, type LanguageKey } from '$lib/state/i18n.svelte';
	import {
		notifications,
		notifySuccess,
		type Level,
		type Mode
	} from '$lib/state/notifications.svelte';
	import { theme, type ThemeChoice } from '$lib/state/theme.svelte';
	import { welcome } from '$lib/state/welcome.svelte';
	import { windowPrefs } from '$lib/state/windowPrefs.svelte';

	/** Пункты выпадающего списка языков: имя на самом языке, подпись — по-английски. */
	const LANGUAGE_OPTIONS = LANGUAGES.map((language) => ({
		value: language.key,
		label: language.name,
		note: language.note
	}));

	/*
	 * Варианты темы в том же порядке, что и подложка переключателя.
	 *
	 * Подпись короткая, полная уходит в подсказку: пункты в переключателе равны
	 * по ширине, и «Синхронизация с компьютером» раздвигала бы ряд втрое.
	 */
	const THEMES: { id: ThemeChoice; ico: string; label: () => string; tip: () => string }[] = [
		{ id: 'White', ico: 'light', label: () => i18n.t.theme_light, tip: () => i18n.t.settings_text_6_1 },
		{ id: 'Black', ico: 'dark', label: () => i18n.t.theme_dark, tip: () => i18n.t.settings_text_6_2 },
		{
			id: 'Systemic',
			ico: 'system',
			label: () => i18n.t.theme_system,
			tip: () => i18n.t.settings_text_6_3
		}
	];

	const themeIndex = $derived(Math.max(0, THEMES.findIndex((t) => t.id === theme.choice)));

	/*
	 * Сколько уведомлений нужно.
	 *
	 * Пояснение показывается только у выбранного режима: три подписи разом —
	 * это стена текста ради строки, которую настраивают один раз.
	 */
	/*
	 * Подписи короткие, полные уходят в подсказку — как у выбора темы.
	 *
	 * Пункты в переключателе равны по ширине и не переносятся, поэтому «Только
	 * важное» либо раздвигало бы ряд, либо обрезалось посередине слова.
	 */
	const NOTE_MODES: {
		id: Mode;
		label: () => string;
		tip: () => string;
		note: () => string;
	}[] = [
		{
			id: 'all',
			label: () => i18n.t.note_all,
			tip: () => i18n.t.note_all,
			note: () => i18n.t.note_all_note
		},
		{
			id: 'quiet',
			label: () => i18n.t.note_quiet,
			tip: () => i18n.t.note_quiet,
			note: () => i18n.t.note_quiet_note
		},
		{
			id: 'alerts',
			label: () => i18n.t.note_alerts,
			tip: () => i18n.t.note_alerts_full,
			note: () => i18n.t.note_alerts_note
		},
		{
			id: 'custom',
			label: () => i18n.t.note_custom,
			tip: () => i18n.t.note_custom,
			note: () => i18n.t.note_custom_note
		}
	];

	const noteIndex = $derived(
		Math.max(
			0,
			NOTE_MODES.findIndex((mode) => mode.id === notifications.mode)
		)
	);

	/** Уровни в порядке возрастания веса — так их и читают сверху вниз. */
	const NOTE_LEVELS: { id: Level; label: () => string; note: () => string }[] = [
		{ id: 'info', label: () => i18n.t.note_l_info, note: () => i18n.t.note_l_info_note },
		{ id: 'success', label: () => i18n.t.note_l_success, note: () => i18n.t.note_l_success_note },
		{ id: 'warning', label: () => i18n.t.note_l_warning, note: () => i18n.t.note_l_warning_note },
		{ id: 'error', label: () => i18n.t.note_l_error, note: () => i18n.t.note_l_error_note }
	];

	/** Сколько окошко висит: секунды, а не миллисекунды — так их и называют. */
	const LINGERS = [3, 5, 8, 12];

	/** Сколько записей хранить. */
	const LIMITS = [20, 50, 100, 200];

	let noteOpen = $state(false);

	function toggleToast(level: Level) {
		notifications.tune({
			toast: { ...notifications.rules.toast, [level]: !notifications.rules.toast[level] }
		});
	}

	function toggleKeep(level: Level) {
		notifications.tune({
			keep: { ...notifications.rules.keep, [level]: !notifications.rules.keep[level] }
		});
	}

	/** Прокручиваемая область: за ней следит бегунок списка разделов. */
	let scroller = $state<HTMLElement | null>(null);


	/*
	 * Переход к нужному месту настроек извне.
	 *
	 * Из панели трея и из меню значка корзины просят открыть не настройки
	 * вообще, а определённый блок в них: длинный список, и искать в нём глазами
	 * то, ради чего пришёл, — не дело.
	 */
	$effect(() => {
		const asked = listen<string>('app:settings-anchor', (event) => {
			document
				.getElementById(event.payload)
				?.scrollIntoView({ behavior: 'smooth', block: 'start' });
		});

		return () => {
			asked.then((stop) => stop());
		};
	});

	const NAV = $derived([
		{ id: 'SettingsHome', label: i18n.t.settings_text_1 },
		{ id: 'SettingsHomeWindows', label: i18n.t.settings_text_1_1 }
	]);

	/** Значки: подпись у каждого своя, картинка — общая с ядром. */
	const ICONS = $derived(
		ICON_ORDER.map((id) => ({
			id,
			src: ICON_PREVIEWS[id],
			label: (i18n.t as Record<string, string>)[`icon_${id}`] ?? id
		}))
	);

	$effect(() => {
		appIcon.load();
	});

	// Настройки окна перечитываются у ядра при каждом входе: реестр могли
	// поправить снаружи, и запомненное значение соврало бы
	$effect(() => {
		windowPrefs.load();
	});

	async function changeLanguage(key: string) {
		i18n.set(key as LanguageKey);
		await invoke('app_update_labels');
	}

	async function setTheme(choice: ThemeChoice) {
		theme.set(choice);
		await invoke('app_update_labels');
	}

	async function pickIcon(id: string) {
		const ok = await appIcon.set(id);
		if (ok) notifySuccess(i18n.t.icon_applied, ICONS.find((icon) => icon.id === id)?.label ?? id);
	}
</script>

<div class="qrx_set">
	<SettingsNav items={NAV} {scroller} />

	<div class="qrx_set_body" bind:this={scroller}>
		<div class="qrx_set_head" id="SettingsHome">{i18n.t.settings_text_1}</div>

		<!-- Язык -->
		<section class="qrx_set_row">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.settings_text_2}</span>
				<span class="qrx_set_row_note">{i18n.t.settings_text_2_1}</span>
			</div>

			<div class="qrx_set_row_controls">
				<Select
					options={LANGUAGE_OPTIONS}
					value={i18n.key}
					onpick={changeLanguage}
					label={i18n.t.settings_text_2_1}
				/>
			</div>
		</section>

		<!--
			Приветствие.

			Мастер показывается один раз, и отметка об этом лежит в хранилище
			окна. Без этой кнопки посмотреть на него второй раз было нельзя ничем,
			кроме как вычистив хранилище целиком — вместе с языком, темой и
			памятью об окне.
		-->
		<section class="qrx_set_row">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.wiz_again}</span>
				<span class="qrx_set_row_note">{i18n.t.wiz_again_note}</span>
			</div>

			<div class="qrx_set_row_controls">
				<button class="qrx_sys_link" onclick={() => welcome.again()}>
					{i18n.t.wiz_again_do}
				</button>
			</div>
		</section>

		<!-- Тема -->
		<section class="qrx_set_row">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.settings_text_3}</span>
				<span class="qrx_set_row_note">{i18n.t.settings_text_3_2}</span>
			</div>

			<div
				class="qrx_seg"
				style="--seg-count: {THEMES.length}; --seg-index: {themeIndex}"
				role="radiogroup"
				aria-label={i18n.t.settings_text_3}
			>
				<div class="qrx_seg_thumb" aria-hidden="true"></div>

				{#each THEMES as option (option.id)}
					<button
						class="qrx_seg_item"
						class:qrx_seg_item_active={theme.choice === option.id}
						role="radio"
						aria-checked={theme.choice === option.id}
						aria-label={option.tip()}
						onclick={() => setTheme(option.id)}
					>
						<span class="qrx_seg_ico qrx_theme_ico_{option.ico}" aria-hidden="true"></span>
						{option.label()}
					</button>
				{/each}
			</div>
		</section>

		<!-- Уведомления: сверху готовый набор, под «Подробнее» — всё поштучно -->
		<section class="qrx_set_row qrx_set_row_stack">
			<div class="qrx_set_row_head">
				<div class="qrx_set_row_text">
					<span class="qrx_set_row_title">{i18n.t.note_mode}</span>
					<!-- Подпись здесь постоянная. Пояснение выбранного режима стоит
					     отдельной строкой ниже: у «Тише» оно длинное и, стоя рядом,
					     выталкивало переключатель на другую строку — ряд прыгал при
					     каждом переключении -->
					<span class="qrx_set_row_note">{i18n.t.note_mode_note}</span>
				</div>

				<div
					class="qrx_seg"
					style="--seg-count: {NOTE_MODES.length}; --seg-index: {noteIndex}"
					role="radiogroup"
					aria-label={i18n.t.note_mode}
				>
					<div class="qrx_seg_thumb" aria-hidden="true"></div>

					{#each NOTE_MODES as option (option.id)}
						<button
							class="qrx_seg_item"
							class:qrx_seg_item_active={notifications.mode === option.id}
							role="radio"
							aria-checked={notifications.mode === option.id}
							aria-label={option.tip()}
							data-tooltip={option.tip()}
							data-tooltip-position="top"
							onclick={() => notifications.setMode(option.id)}
						>
							{option.label()}
						</button>
					{/each}
				</div>
			</div>

			<!-- Пояснение выбранного режима: своя строка во всю ширину, ничего не
			     двигает и меняется без прыжков -->
			<p class="qrx_note_hint">{NOTE_MODES[noteIndex].note()}</p>

			<div class="qrx_iso_actions">
				<button class="qrx_rec_log" onclick={() => (noteOpen = !noteOpen)}>
					{noteOpen ? i18n.t.iso_less : i18n.t.iso_more}
				</button>

				<button
					class="qrx_rec_log qrx_lib_drop"
					disabled={!notifications.any}
					onclick={() => notifications.clear()}
				>
					{i18n.t.note_clear} ({notifications.items.length})
				</button>
			</div>

			{#if noteOpen}
				<div class="qrx_iso_deep">
					<!-- Две колонки не случайно: окошко и список — разные судьбы у
					     одного события, и настраивать их надо порознь -->
					<div class="qrx_note_grid">
						<span class="qrx_field_label"></span>
						<span class="qrx_field_label">{i18n.t.note_toast}</span>
						<span class="qrx_field_label">{i18n.t.note_keep}</span>

						{#each NOTE_LEVELS as level (level.id)}
							<div class="qrx_set_row_text">
								<span class="qrx_iso_cap_name">{level.label()}</span>
								<span class="qrx_rec_task_note">{level.note()}</span>
							</div>

							<label class="qrx_note_cell">
								<input
									class="qrx_toggle"
									type="checkbox"
									checked={notifications.rules.toast[level.id]}
									onchange={() => toggleToast(level.id)}
								/>
							</label>

							<label class="qrx_note_cell">
								<input
									class="qrx_toggle"
									type="checkbox"
									checked={notifications.rules.keep[level.id]}
									onchange={() => toggleKeep(level.id)}
								/>
							</label>
						{/each}
					</div>

					<div class="qrx_iso_group">
						<span class="qrx_field_label">{i18n.t.note_linger}</span>
						<span class="qrx_rec_task_note">{i18n.t.note_linger_note}</span>

						<div class="qrx_found_kinds">
							{#each LINGERS as seconds (seconds)}
								<button
									class="qrx_found_chip"
									class:qrx_found_chip_active={notifications.rules.linger === seconds * 1000}
									onclick={() => notifications.tune({ linger: seconds * 1000 })}
								>
									{seconds} {i18n.t.note_seconds}
								</button>
							{/each}
						</div>
					</div>

					<div class="qrx_iso_group">
						<span class="qrx_field_label">{i18n.t.note_limit}</span>
						<span class="qrx_rec_task_note">{i18n.t.note_limit_note}</span>

						<div class="qrx_found_kinds">
							{#each LIMITS as count (count)}
								<button
									class="qrx_found_chip"
									class:qrx_found_chip_active={notifications.rules.limit === count}
									onclick={() => notifications.tune({ limit: count })}
								>
									{count}
								</button>
							{/each}
						</div>
					</div>
				</div>
			{/if}
		</section>

		<!-- Корзина. Настройки живут только здесь: в панели трея их нет вовсе,
		     она показывает состояние и два действия.

		     id нужен для перехода из трея: оттуда просят открыть не просто
		     настройки, а это место в них -->
		<section class="qrx_set_row qrx_set_row_stack" id="SettingsBin">
			<div class="qrx_set_row_head">
				<div class="qrx_set_row_text">
					<span class="qrx_set_row_title">{i18n.t.appTrayText_1_6}</span>
					<span class="qrx_set_row_note">{i18n.t.bin_tray_note}</span>
				</div>
			</div>

			<BinTray />
		</section>

		<!-- Значок приложения -->
		<section class="qrx_set_row qrx_set_row_stack">
			<div class="qrx_set_row_head">
				<div class="qrx_set_row_text">
					<span class="qrx_set_row_title">{i18n.t.settings_text_4}</span>
					<span class="qrx_set_row_note">{i18n.t.settings_text_4_1}</span>
				</div>
			</div>

			<div class="qrx_icons">
				{#each ICONS as icon (icon.id)}
					<button
						class="qrx_icon"
						class:qrx_icon_active={appIcon.current === icon.id}
						aria-pressed={appIcon.current === icon.id}
						aria-label={icon.label}
						onclick={() => pickIcon(icon.id)}
					>
						<img class="qrx_icon_img" src={icon.src} alt="" />
						<span class="qrx_icon_label">{icon.label}</span>
					</button>
				{/each}
			</div>

			<label class="qrx_icon_extra">
				<span class="qrx_set_row_text">
					<span class="qrx_set_row_title">{i18n.t.icon_shortcut}</span>
					<span class="qrx_set_row_note">{i18n.t.icon_shortcut_note}</span>
				</span>
				<input type="checkbox" bind:checked={appIcon.withShortcuts} />
			</label>
		</section>

		<div class="qrx_set_head" id="SettingsHomeWindows">{i18n.t.settings_text_1_1}</div>

		<!-- Запуск -->
		<section class="qrx_set_row">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.settings_text_8}</span>
			</div>
			<input
				class="qrx_toggle"
				type="checkbox"
				checked={windowPrefs.autostart}
				disabled={windowPrefs.busy}
				onchange={(event) => windowPrefs.setAutostart(event.currentTarget.checked)}
			/>
		</section>

		<!--
			Свёрнутый старт имеет смысл только вместе с автозапуском: признак
			дописывается в ту самую запись реестра. Поэтому строка вложена.
		-->
		<section class="qrx_set_row qrx_set_row_child" class:disabled={!windowPrefs.autostart}>
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.settings_text_8_1}</span>
			</div>
			<input
				class="qrx_toggle"
				type="checkbox"
				checked={windowPrefs.startMinimized}
				disabled={windowPrefs.busy}
				onchange={(event) => windowPrefs.setStartMinimized(event.currentTarget.checked)}
			/>
		</section>

		<!-- Закрытие -->
		<section class="qrx_set_row">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.settings_text_8_2}</span>
				<span class="qrx_set_row_note">
					{windowPrefs.closeToTray ? i18n.t.win_close_tray_note : i18n.t.win_close_quit}
				</span>
			</div>
			<input
				class="qrx_toggle"
				type="checkbox"
				checked={windowPrefs.closeToTray}
				disabled={windowPrefs.busy}
				onchange={(event) => windowPrefs.setCloseToTray(event.currentTarget.checked)}
			/>
		</section>

		<!-- Возвращаться на тот же раздел -->
		<section class="qrx_set_row">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.win_keep_view}</span>
				<span class="qrx_set_row_note">
					{windowPrefs.rememberView ? i18n.t.win_keep_view_on : i18n.t.win_keep_view_off}
				</span>
			</div>
			<input
				class="qrx_toggle"
				type="checkbox"
				checked={windowPrefs.rememberView}
				disabled={windowPrefs.busy}
				onchange={(event) => windowPrefs.setRememberView(event.currentTarget.checked)}
			/>
		</section>

		<!-- Поверх других окон -->
		<section class="qrx_set_row">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.win_always_on_top}</span>
				<span class="qrx_set_row_note">{i18n.t.win_always_on_top_note}</span>
			</div>
			<input
				class="qrx_toggle"
				type="checkbox"
				checked={windowPrefs.alwaysOnTop}
				disabled={windowPrefs.busy}
				onchange={(event) => windowPrefs.setAlwaysOnTop(event.currentTarget.checked)}
			/>
		</section>

		<!-- Положение окна -->
		<section class="qrx_set_row">
			<div class="qrx_set_row_text">
				<span class="qrx_set_row_title">{i18n.t.win_remember}</span>
				<span class="qrx_set_row_note">{i18n.t.win_remember_note}</span>
			</div>
			<input
				class="qrx_toggle"
				type="checkbox"
				checked={windowPrefs.rememberGeometry}
				disabled={windowPrefs.busy}
				onchange={(event) => windowPrefs.setRememberGeometry(event.currentTarget.checked)}
			/>
		</section>
	</div>
</div>
