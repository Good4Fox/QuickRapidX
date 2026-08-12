<script lang="ts">
	/*
	 * Приветственный мастер первого запуска.
	 *
	 * Сделан по образцу приветственного окна Windows 11 — того, что настраивает
	 * систему при первом запуске. Взяты оттуда не цвета, а устройство: узкая
	 * колонка посреди огромного пустого поля, три ступени размера шрифта и ни
	 * одной больше, один акцент на экран, кнопки в правом нижнем углу.
	 * Подробности — в комментарии к `welcome.css`.
	 *
	 * Пять шагов. Первый и последний — паузы: крупный знак, крупная строка,
	 * одна кнопка. Три средних — настоящая настройка: чем пользуются, как
	 * выглядит программа и что поправить в самой Windows.
	 *
	 * Каждый средний шаг что-то ДЕЛАЕТ. Прежде здесь был один шаг с галочками,
	 * которые не читались нигде: человек отмечал «Игры», а программа делала
	 * вид, что не заметила.
	 *
	 * Музыка подключается импортом, а не путём `/src/lib/App/sound/...`: такой
	 * адрес существует только у dev-сервера, в собранном приложении файла по
	 * нему нет и звук просто не игрался.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import { fade, fly, scale } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';

	import soundUrl from '$lib/App/sound/create_settings/sound_1_320.mp3';
	import { setAppFrame } from '$lib/state/appFrame';
	import { i18n, LANGUAGES } from '$lib/state/i18n.svelte';
	import { notifyError } from '$lib/state/notifications.svelte';
	import { theme, type ThemeChoice } from '$lib/state/theme.svelte';
	import { saveUses, type Use } from '$lib/state/welcome.svelte';
	import { windowPrefs } from '$lib/state/windowPrefs.svelte';

	type Props = {
		/** Мастер пройден — вернуться к обычному интерфейсу. */
		ondone: () => void;
	};

	let { ondone }: Props = $props();

	const appWindow = getCurrentWindow();

	/** Порядок шагов. Номера — коды, а не места в ряду. */
	const ORDER = [1, 3, 4, 5, 7] as const;

	type Step = (typeof ORDER)[number];

	let step = $state<Step>(1);

	let audio: HTMLAudioElement | null = null;
	let playing = $state(false);
	let volume = $state(3);

	/*
	 * Движение убрано в самой системе.
	 *
	 * У Windows это не «замедлим вдвое», а «либо есть, либо нет»: переходы
	 * становятся мгновенными. Считается один раз — настройка меняется раз в
	 * жизни, и держать ради неё слушателя незачем.
	 */
	const still =
		typeof window !== 'undefined' &&
		window.matchMedia('(prefers-reduced-motion: reduce)').matches;

	/*
	 * Смена шага.
	 *
	 * Уходящий гаснет за сто пятьдесят миллисекунд и никуда не уезжает.
	 * Приходящий въезжает снизу на двадцать восемь точек за триста, начиная
	 * чуть раньше, чем прошлый догорел, — от этого перекрытия смена и читается
	 * как одно движение, а не как две отдельные.
	 *
	 * Кривая с резким стартом и длинным торможением — подпись Windows 11:
	 * движение ощущается почти законченным, едва начавшись. Блок едет целиком,
	 * карточки по очереди не разъезжаются: у системы такого нет нигде.
	 */
	const goes = { duration: still ? 0 : 150 };
	const comes = { y: 28, duration: still ? 0 : 300, delay: still ? 0 : 120, easing: cubicOut };

	/** Появление самого мастера: знак подрастает, всё остальное проявляется. */
	const opens = { duration: still ? 0 : 420, start: 0.94, opacity: 0, easing: cubicOut };

	/*
	 * Способы использования устройства — шесть карточек.
	 *
	 * Подписи берутся из словаря, а не лежат здесь строками: мастер
	 * показывается раньше, чем человек доберётся до выбора языка.
	 */
	const USES: { id: Use; ico: string; title: string; text: string }[] = $derived([
		{ id: 'fun', ico: 'entertainment', title: i18n.t.wiz_use1_title, text: i18n.t.wiz_use1_text },
		{ id: 'games', ico: 'games', title: i18n.t.wiz_use2_title, text: i18n.t.wiz_use2_text },
		{ id: 'school', ico: 'school', title: i18n.t.wiz_use3_title, text: i18n.t.wiz_use3_text },
		{ id: 'making', ico: 'creativity', title: i18n.t.wiz_use4_title, text: i18n.t.wiz_use4_text },
		{ id: 'work', ico: 'business', title: i18n.t.wiz_use5_title, text: i18n.t.wiz_use5_text },
		{ id: 'family', ico: 'family', title: i18n.t.wiz_use6_title, text: i18n.t.wiz_use6_text }
	]);

	/** Значки карточек лежат отдельными файлами — собираем путь по имени. */
	const ICONS = import.meta.glob<string>('$lib/App/img/icons/Creating_interaction/*.svg', {
		eager: true,
		query: '?url',
		import: 'default'
	});

	function iconOf(name: string): string {
		const found = Object.entries(ICONS).find(([path]) => path.endsWith(`/${name}.svg`));

		return found ? found[1] : '';
	}

	let picked = $state<Use[]>([]);

	function toggleUse(id: Use) {
		picked = picked.includes(id) ? picked.filter((each) => each !== id) : [...picked, id];
	}

	const THEMES: { id: ThemeChoice; label: () => string; note: () => string }[] = [
		{ id: 'White', label: () => i18n.t.theme_light, note: () => i18n.t.settings_text_6_1 },
		{ id: 'Black', label: () => i18n.t.theme_dark, note: () => i18n.t.settings_text_6_2 },
		{ id: 'Systemic', label: () => i18n.t.theme_system, note: () => i18n.t.settings_text_6_3 }
	];

	/*
	 * Приглашения по горячим сочетаниям.
	 *
	 * Пять нажатий Shift подряд, зажатый Shift, Num Lock — Windows на каждое
	 * отвечает окном с вопросом, и ловится это чаще всего в играх и за
	 * набором текста. Три ключа реестра, и все три уже умеет ядро.
	 */
	let prompts = $state(true);
	let busy = $state(false);

	async function setPrompts(on: boolean) {
		if (busy) return;

		prompts = on;
		busy = true;

		try {
			await Promise.all([
				invoke('tweaks_set_sticky', { on }),
				invoke('tweaks_set_filter', { on }),
				invoke('tweaks_set_toggle', { on })
			]);
		} catch (e) {
			// Возвращаем переключатель: показывать выключенным то, что не
			// выключилось, — врать человеку
			prompts = !on;
			notifyError(i18n.t.notification_text_4, String(e));
		}

		busy = false;
	}

	$effect(() => {
		audio = new Audio(soundUrl);
		audio.loop = true;
		audio.volume = volume / 100;

		// Автовоспроизведение до первого нажатия движок может отклонить —
		// тогда просто остаёмся в выключенном состоянии, без ошибки в консоли.
		audio
			.play()
			.then(() => (playing = true))
			.catch(() => (playing = false));

		windowPrefs.load();

		// Нынешнее состояние приглашений: показывать переключатель наугад
		// значило бы предложить выключить то, что и так выключено
		invoke<{ sticky_hotkey: boolean; filter_hotkey: boolean; toggle_hotkey: boolean }>('tweaks_get')
			.then((tweaks) => {
				prompts = tweaks.sticky_hotkey || tweaks.filter_hotkey || tweaks.toggle_hotkey;
			})
			.catch(() => undefined);

		return () => {
			audio?.pause();
			audio = null;
		};
	});

	function toggleSound() {
		if (!audio) return;

		if (playing) audio.pause();
		else audio.play();

		playing = !playing;
	}

	function changeVolume(event: Event) {
		volume = Number((event.target as HTMLInputElement).value);
		if (audio) audio.volume = volume / 100;
	}

	/** Вперёд по ряду шагов. */
	function next() {
		if (step === 3) saveUses(picked);

		const at = ORDER.indexOf(step);

		step = ORDER[Math.min(at + 1, ORDER.length - 1)];
	}

	/** Первый шаг разворачивает окно на весь экран и убирает рамку. */
	async function start() {
		await appWindow.maximize();
		setAppFrame(false);
		next();
	}

	/** Последний шаг возвращает окно и запоминает, что мастер пройден. */
	async function finish() {
		audio?.pause();
		playing = false;

		setAppFrame(true);
		await appWindow.unmaximize();

		ondone();
	}
</script>

<div class="qrx_wiz" in:fade={{ duration: still ? 0 : 260 }}>
	<!--
		Выбор языка виден с первого кадра.

		Мастер показывается раньше, чем человек доберётся до настроек, и до сих
		пор поменять язык на нём было нельзя ничем. Переключателем, а не
		списком: языков три, и все три помещаются в строку — тому, кто открыл
		программу на чужом языке, надо увидеть своё название сразу, а не искать
		его за нажатием.
	-->
	<div class="qrx_wiz_lang">
		{#each LANGUAGES as language (language.key)}
			<button
				class="qrx_wiz_lang_item"
				class:qrx_wiz_lang_on={i18n.key === language.key}
				lang={language.tag}
				aria-pressed={i18n.key === language.key}
				onclick={() => i18n.set(language.key)}
			>
				{language.name}
			</button>
		{/each}
	</div>

	<div class="qrx_wiz_sound">
		<!-- Шкала всегда в разметке, а не появляется по условию: ширину нельзя
		     анимировать у того, чего в прошлом кадре не было -->
		<div class="qrx_wiz_sound_slot">
			{#if playing}
				<input
					class="qrx_wiz_sound_slider"
					style="--fill: {volume * 10}%"
					type="range"
					min="0"
					max="10"
					aria-label={i18n.t.wiz_volume}
					data-tooltip={volume * 10 + '%'}
					data-tooltip-position="bottom"
					value={volume}
					oninput={changeVolume}
				/>
			{/if}
		</div>

		<button
			class="qrx_wiz_sound_button"
			class:qrx_wiz_sound_on={playing}
			class:qrx_wiz_sound_off={!playing}
			aria-label={playing ? i18n.t.wiz_sound_off : i18n.t.wiz_sound_on}
			onclick={toggleSound}
		></button>
	</div>

	<!--
		Уходящий и приходящий шаги лежат в одной клетке сетки.

		Иначе на время смены они встают друг под друга, высота скачком
		удваивается, и содержимое уезжает вверх прямо посреди перехода.
	-->
	<div class="qrx_wiz_body">
		{#key step}
			<div class="qrx_wiz_stage" in:fly={comes} out:fade={goes}>
				{#if step === 1}
					<div class="qrx_wiz_mark" in:scale={opens}></div>

					<div class="qrx_wiz_head">
						<h1 class="qrx_wiz_title">{i18n.t.wiz_hello}</h1>
						<p class="qrx_wiz_lead">{i18n.t.wiz_hello_note}</p>
					</div>
				{:else if step === 3}
					<div class="qrx_wiz_head">
						<h1 class="qrx_wiz_title">{i18n.t.wiz_uses_title}</h1>
						<p class="qrx_wiz_lead">{i18n.t.wiz_uses_note}</p>
					</div>

					<!--
						Карточка нажимается целиком, а не только флажок: так у самой
						системы, и попасть в двадцать точек флажка куда труднее, чем
						в карточку. Флажок здесь — вид, а не орган управления,
						поэтому он `<span>`, а состояние держит `aria-pressed`.
					-->
					<div class="qrx_wiz_cards">
						{#each USES as use (use.id)}
							<button
								class="qrx_wiz_card"
								class:qrx_wiz_card_on={picked.includes(use.id)}
								aria-pressed={picked.includes(use.id)}
								onclick={() => toggleUse(use.id)}
							>
								<span
									class="qrx_wiz_card_ico"
									style="background-image: url({iconOf(use.ico)})"
								></span>

								<span class="qrx_wiz_card_text">
									<span class="qrx_wiz_card_name">{use.title}</span>
									<span class="qrx_wiz_card_note">{use.text}</span>
								</span>

								<span class="qrx_wiz_check"></span>
							</button>
						{/each}
					</div>
				{:else if step === 4}
					<div class="qrx_wiz_head">
						<h1 class="qrx_wiz_title">{i18n.t.wiz_look_title}</h1>
						<p class="qrx_wiz_lead">{i18n.t.wiz_look_note}</p>
					</div>

					<!-- Тема выбирается одна из трёх, поэтому карточки без флажка:
					     выбранная показана обводкой акцентом, как переключатель -->
					<div class="qrx_wiz_themes">
						{#each THEMES as option (option.id)}
							<button
								class="qrx_wiz_theme qrx_wiz_theme_{option.id}"
								class:qrx_wiz_theme_on={theme.choice === option.id}
								aria-pressed={theme.choice === option.id}
								onclick={() => theme.set(option.id)}
							>
								<span class="qrx_wiz_theme_view"></span>
								<span class="qrx_wiz_card_name">{option.label()}</span>
								<span class="qrx_wiz_card_note">{option.note()}</span>
							</button>
						{/each}
					</div>
				{:else if step === 5}
					<div class="qrx_wiz_head">
						<h1 class="qrx_wiz_title">{i18n.t.wiz_tune_title}</h1>
						<p class="qrx_wiz_lead">{i18n.t.wiz_tune_note}</p>
					</div>

					<div class="qrx_wiz_rows">
						<label class="qrx_wiz_row">
							<span class="qrx_wiz_card_text">
								<span class="qrx_wiz_card_name">{i18n.t.wiz_prompts}</span>
								<span class="qrx_wiz_card_note">{i18n.t.wiz_prompts_note}</span>
							</span>

							<input
								class="qrx_toggle"
								type="checkbox"
								checked={!prompts}
								disabled={busy}
								onchange={(event) => setPrompts(!event.currentTarget.checked)}
							/>
						</label>

						<label class="qrx_wiz_row">
							<span class="qrx_wiz_card_text">
								<span class="qrx_wiz_card_name">{i18n.t.wiz_autostart}</span>
								<span class="qrx_wiz_card_note">{i18n.t.wiz_autostart_note}</span>
							</span>

							<input
								class="qrx_toggle"
								type="checkbox"
								checked={windowPrefs.autostart}
								disabled={windowPrefs.busy}
								onchange={(event) => windowPrefs.setAutostart(event.currentTarget.checked)}
							/>
						</label>

						<label class="qrx_wiz_row">
							<span class="qrx_wiz_card_text">
								<span class="qrx_wiz_card_name">{i18n.t.wiz_tray}</span>
								<span class="qrx_wiz_card_note">{i18n.t.wiz_tray_note}</span>
							</span>

							<input
								class="qrx_toggle"
								type="checkbox"
								checked={windowPrefs.closeToTray}
								disabled={windowPrefs.busy}
								onchange={(event) => windowPrefs.setCloseToTray(event.currentTarget.checked)}
							/>
						</label>
					</div>
				{:else}
					<div class="qrx_wiz_mark" in:scale={opens}></div>

					<div class="qrx_wiz_head">
						<h1 class="qrx_wiz_title">{i18n.t.wiz_done}</h1>
						<p class="qrx_wiz_lead">{i18n.t.wiz_done_note}</p>
					</div>
				{/if}
			</div>
		{/key}
	</div>

	<!--
		Полоса кнопок стоит вне сменяемого блока: она одна на все шаги, и её
		переезд при смене шага читался бы как подмена, а не как продолжение.
	-->
	<div class="qrx_wiz_bar">
		{#if step === 1}
			<!-- Крупнее прочих: на экране, где кроме знака и одной строки ничего
			     нет, обычная кнопка теряется -->
			<button class="qrx_wiz_button qrx_wiz_button_main qrx_wiz_button_hero" onclick={start}>
				{i18n.t.wiz_start}
			</button>
		{:else if step === 7}
			<button class="qrx_wiz_button qrx_wiz_button_main qrx_wiz_button_hero" onclick={finish}>
				{i18n.t.wiz_finish}
			</button>
		{:else}
			<!-- Отказной путь нужен не для симметрии: он и делает главную кнопку
			     главной. У Windows рядом с «Далее» всегда есть «Пропустить» -->
			<button class="qrx_wiz_button" onclick={next}>
				{i18n.t.wiz_skip}
			</button>

			<button class="qrx_wiz_button qrx_wiz_button_main" onclick={next}>
				{step === 3 ? i18n.t.wiz_accept : i18n.t.wiz_next}
			</button>
		{/if}
	</div>
</div>
