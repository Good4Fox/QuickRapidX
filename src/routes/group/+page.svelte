<script lang="ts">
	/*
	 * Окно набора — то, что открывается по значку в трее и по ярлыку.
	 *
	 * Своё окно, а не панель трея. Панель — это ящик с разделами: корзина,
	 * общий список, нижний ряд переходов. Набор же открывают ради одного
	 * действия: увидеть свои программы и запустить нужную. Делить одно окно на
	 * две такие задачи значит либо тащить в набор лишнее, либо ломать панель.
	 *
	 * Закрывается по уходу фокуса, Esc и после запуска. Уход фокуса игнорируется
	 * первые мгновения после показа: Windows успевает прислать запоздалое
	 * «фокуса нет» от окна, которое его ещё не получало, и окно закрывалось бы
	 * сразу после открытия.
	 */
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { getCurrentWindow } from '@tauri-apps/api/window';

	import { flip } from 'svelte/animate';
	import { expoOut } from 'svelte/easing';

	import AppIcon from '$lib/components/ui/AppIcon.svelte';
	import { i18n } from '$lib/state/i18n.svelte';
	import { revive } from '$lib/state/icons';
	import { groups, type Group, type Shortcut } from '$lib/state/groups.svelte';

	const panel = getCurrentWindow();

	let open_id = $state('');
	let trail = $state<string[]>([]);
	let failed = $state('');

	/*
	 * Номер показа.
	 *
	 * Окно не закрывается, а прячется, и разметка между показами живёт. Значит,
	 * появление само собой не повторится: плитки уже на месте. Номер меняется
	 * на каждый показ, разметка по нему пересоздаётся — и значки влетают заново.
	 */
	let shows = $state(0);

	/** Какую плитку тащат. */
	let held = $state(-1);

	/** Порядок во время переноса: список расступается под рукой. */
	let preview = $state<Shortcut[] | null>(null);


	const current = $derived(groups.list.find((group) => group.id === open_id) ?? null);

	/** Список ещё не поднят — набора «нет» пока рано говорить. */
	let ready = $state(false);

	/*
	 * С какой стороны стоит панель задач.
	 *
	 * Окно вырастает от края, который на неё смотрит, — иначе непонятно, откуда
	 * оно взялось. Спрашиваем у ядра: у окна нет ни положения панели, ни своего
	 * места на экране.
	 */
	let edge = $state('bottom');

	/*
	 * Точка роста и короткий ход в ту же сторону.
	 *
	 * Ход именно короткий, шесть пикселей: длинный полёт от панели задач — это
	 * системная всплывашка, а наше окно и так стоит вплотную к своему значку, и
	 * лететь ему неоткуда.
	 */
	const grow = $derived.by(() => {
		switch (edge) {
			case 'top':
				return { origin: 'top center', x: '0px', y: '-6px' };
			case 'left':
				return { origin: 'center left', x: '-6px', y: '0px' };
			case 'right':
				return { origin: 'center right', x: '6px', y: '0px' };
			default:
				return { origin: 'bottom center', x: '0px', y: '6px' };
		}
	});

	/*
	 * Движение убрано в самой системе.
	 *
	 * Считается один раз: окно живёт весь сеанс, а настройка меняется раз в
	 * жизни, и подписываться на неё здесь значило бы держать слушателя ради
	 * события, которого не будет.
	 */
	const still =
		typeof window !== 'undefined' &&
		window.matchMedia('(prefers-reduced-motion: reduce)').matches;

	$effect(() => {
		i18n.restore();

		// Окно не перезагружается ни разу за сеанс, и всё, что застряло в
		// прошлый показ, застряло бы навсегда. Показ — единственный миг, когда
		// очередь значков можно поднять заново
		revive();

		/*
		 * Спрашиваем, что показывать, а не только ждём события.
		 *
		 * Событие уходит один раз, перед показом окна. При первом открытии
		 * интерфейс в этот момент ещё грузится и подписаться не успевает — и
		 * окно оставалось ни с чем, честно сообщая, что набор удалён. Поэтому
		 * при появлении оно спрашивает состояние само.
		 */
		invoke<string>('tray_edge')
			.then((value) => (edge = value))
			.catch(() => undefined);

		Promise.all([groups.reload(), invoke<string>('tray_opened_group').catch(() => '')]).then(
			([, asked]) => {
				if (!open_id && asked) open_id = asked;

				ready = true;
			}
		);

		const opened = listen<string>('tray:group', (event) => {
			failed = '';
			trail = [];
			preview = null;
			held = -1;
			open_id = event.payload ?? '';
			shows += 1;

			// Значки: то же, что и при первом показе. Запрос, застигнутый
			// усыплением, иначе занимал бы место в очереди до конца сеанса
			revive();
			groups.reload();
		});

		const keys = (event: KeyboardEvent) => {
			if (event.key !== 'Escape') return;

			if (trail.length > 0) back();
			else invoke('window_doze');
		};

		window.addEventListener('keydown', keys);

		return () => {
			opened.then((stop) => stop());
			window.removeEventListener('keydown', keys);
		};
	});

	function enter(id: string) {
		if (open_id) trail = [...trail, open_id];

		open_id = id;
	}

	function back() {
		const previous = trail[trail.length - 1];

		trail = trail.slice(0, -1);
		open_id = previous ?? '';
	}

	function icon(group: Group): string {
		return group.icon || group.items[0]?.path || '';
	}

	/*
	 * Окно подгоняется под содержимое.
	 *
	 * Постоянного размера у него нет намеренно: набор из двух ярлыков и набор из
	 * двадцати — разные окна. Так же устроен и AppGroup: там ширина набирается
	 * из плиток и числа колонок, а не задаётся числом.
	 *
	 * Ширина считается по числу плиток в ряду, высота — измерением: сколько
	 * вышло рядов и какой длины подписи, заранее не знает никто. Меряем после
	 * отрисовки, потому и в два шага.
	 */
	/*
	 * Окно размером с горсть значков, не больше.
	 *
	 * Было 84 на значок 56 — по четырнадцать пикселей пустоты с каждой стороны,
	 * и на четырёх программах окно выходило под четыреста в ширину. Теперь
	 * плитка 48 на значок 28: ширина окна считается по плитке и от размера
	 * значка не зависит — уменьшать одно, чтобы уменьшить другое, не нужно.
	 */
	const TILE = 48;
	const GAP = 4;

	// Поля панели, её рамка и рамка самого окна: `#app` тоже съедает по точке с
	// каждой стороны, и без них ряду всегда не хватало двух точек по ширине.
	// По высоте эта нехватка прежде гасилась слагаемым «плюс два» при замере
	const PAD = 6 * 2 + 2 * 2 + 1 * 2;

	/*
	 * Всё в одну строку, без переноса вниз.
	 *
	 * Предел всё же нужен: набор из тридцати программ растянул бы окно шире
	 * экрана. За ним строка начинает прокручиваться вбок, а не заворачиваться.
	 */
	const MAX_COLUMNS = 9;

	let shell = $state<HTMLElement | null>(null);

	/**
	 * Что уже отправлено в ядро.
	 *
	 * Тот же размер второй раз — это лишняя правка окна и заново вырезанная
	 * область, то есть щелчок в конце каждого переноса.
	 */
	let fitted = '';

	$effect(() => {
		const count = current?.items.length ?? 0;
		// Шапка и подписи меняют высоту — пересчитываем и по ним
		const _ = `${current?.head}${current?.labels}${current?.vertical}${shows}`;
		const node = shell;

		if (!node) return;

		/*
		 * Столбцом окно узкое: одна плитка в ширину, сколько бы записей ни было.
		 * Строкой — наоборот. Высоту в обоих случаях меряем, а не считаем: её
		 * задают подписи, а какой они длины, заранее не знает никто.
		 */
		const columns = current?.vertical ? 1 : Math.max(1, Math.min(MAX_COLUMNS, count || 1));
		const width = PAD + columns * TILE + (columns - 1) * GAP;

		let frame = requestAnimationFrame(() => {
			// Второй кадр: к нему разметка уже переложилась под новую ширину
			frame = requestAnimationFrame(() => {
				const height = Math.ceil(node.scrollHeight) + PAD;

				/*
				 * Номер показа входит в отметку намеренно: `tray_fit_group` не
				 * только меняет размер, но и ставит окно на место и заново
				 * вырезает его область — на каждом показе это обязано отработать.
				 */
				const mark = `${shows}:${width}x${height}`;

				if (mark === fitted) return;

				fitted = mark;

				invoke('tray_fit_group', { width, height }).catch(() => undefined);
			});
		});

		return () => cancelAnimationFrame(frame);
	});

	async function launchOne(group: string, item: string) {
		const result = await groups.launch(group, item);

		if (result.ok) invoke('window_doze');
		else failed = result.message;
	}

	/** Что показывать: предпросмотр во время переноса, иначе сам набор. */
	const rows = $derived(preview ?? current?.items ?? []);

	/*
	 * Перетаскивание значков внутри окна.
	 *
	 * На событиях указателя, а не на встроенном переносе разметки: у окон
	 * приложения включён приём файлов извне, и WebView2 забирает себе весь
	 * `dragstart` — тот же случай, что и в списке записей набора.
	 *
	 * Порог в пять пикселей отделяет перенос от нажатия. Без него любой щелчок
	 * по значку считался бы началом переноса, и запустить программу стало бы
	 * нельзя: окно и заведено ради этого нажатия.
	 */
	const THRESHOLD = 5;

	/** Палец дрожит на восемь-десять точек: пять срывало бы каждое касание. */
	const THRESHOLD_TOUCH = 10;

	/** Полоса у края, за которой ряд едет сам. */
	const EDGE = 16;

	/** Дрожание руки: Windows считает непреднамеренным сдвиг до четырёх точек. */
	const HYST = 4;

	/**
	 * Самоход у края: четыреста восемьдесят точек в секунду.
	 *
	 * Считается временем, а не кадрами. Прежние восемь точек за кадр дают те же
	 * четыреста восемьдесят при шестидесяти герцах — и полторы тысячи на экране
	 * в сто шестьдесят пять.
	 */
	const SPEED = 480;

	/*
	 * Сколько ехать соседу.
	 *
	 * Темп взят из главного окна: там строка проезжает семьдесят шесть точек за
	 * двести двадцать миллисекунд — триста сорок пять точек в секунду. Один шаг
	 * ряда по этому темпу и есть сто пятьдесят миллисекунд.
	 *
	 * Рывком рука иногда перебрасывает плитку сразу через несколько мест, и путь
	 * выходит длиннее. Длительность растёт логарифмом, а не прямо: прямо на весь
	 * ряд вышло бы больше секунды. Потолок — триста, та же долгая длительность,
	 * что уже есть в проекте.
	 */
	const slide = (away: number) => Math.min(300, 80 + 70 * Math.log2(away / pitch + 1));

	/** Середины мест в ряду. Меряются один раз за перенос. */
	let slots: { x: number; y: number }[] = [];

	/** Коробка сетки — она же то, что режет. Снимается один раз за перенос. */
	let frame_box = { left: 0, top: 0, right: 0, bottom: 0 };

	/**
	 * Шаг мест и половина плитки.
	 *
	 * Измеряются, а не считаются из TILE и GAP: `box-sizing` у плитки не задан,
	 * и при ширине сорок восемь с полями по три она занимает пятьдесят четыре.
	 */
	let pitch = TILE + GAP;
	let half = { x: TILE / 2, y: TILE / 2 };

	/** Прокрутка на миг замера: по её сдвигу правятся места. */
	let scrolled = { x: 0, y: 0 };

	/** Сетка: на ней держится захват указателя и по ней считается прокрутка. */
	let grid = $state<HTMLElement | null>(null);

	/** Ход взятой плитки за указателем. */
	let lift = $state({ x: 0, y: 0 });

	/** Когда закончился перенос — по этому и глушится ближайшее нажатие. */
	let settled = 0;

	/*
	 * Места меряются один раз за перенос.
	 *
	 * Плитки не меняют ни числа, ни размера — переезжает только их содержимое,
	 * поэтому середины мест постоянны. Прежний расчёт брал их заново на каждое
	 * событие указателя: девять `getBoundingClientRect` по сотне раз в секунду,
	 * и каждый — принудительный пересчёт раскладки.
	 */
	function measure(node: HTMLElement) {
		const box = node.getBoundingClientRect();

		// Коробка сетки за перенос не меняется: подгонка окна зависит от числа
		// записей и вида набора, а они во время переноса стоят на месте
		frame_box = { left: box.left, top: box.top, right: box.right, bottom: box.bottom };
		scrolled = { x: node.scrollLeft, y: node.scrollTop };

		const boxes = Array.from(node.querySelectorAll<HTMLElement>('[data-tile]')).map((tile) =>
			tile.getBoundingClientRect()
		);

		slots = boxes.map((at) => ({ x: at.left + at.width / 2, y: at.top + at.height / 2 }));

		if (boxes[0]) half = { x: boxes[0].width / 2, y: boxes[0].height / 2 };

		// Шаг берётся из самих мест, а не из TILE + GAP: у плитки content-box, и
		// настоящая её ширина на шесть точек больше заданной
		const step =
			slots.length > 1
				? current?.vertical
					? slots[1].y - slots[0].y
					: slots[1].x - slots[0].x
				: 0;

		pitch = step || TILE + GAP;
	}

	/*
	 * Куда встанет плитка.
	 *
	 * Считается не по указателю, а по спроецированной середине самой плитки: где
	 * она сейчас нарисована, туда и метит. Прежний расчёт сравнивал указатель с
	 * серединами ЧУЖИХ мест, и порог обмена зависел от того, за какой край
	 * плитки взялись, — от двадцати восьми точек до семидесяти шести. Теперь
	 * порог всегда один: полшага. Четыре точки сверху — дрожание руки, без них
	 * место щёлкало бы туда-обратно на самой границе.
	 *
	 * Исключать взятую плитку из счёта больше не нужно: места — это геометрия
	 * ряда, а не его содержимое, и от нынешней раскладки расчёт не зависит.
	 */
	function rankOf(p: number): number {
		if (slots.length === 0) return 0;

		const base = current?.vertical ? slots[0].y : slots[0].x;

		if (Math.abs(p - (base + held * pitch)) <= pitch / 2 + HYST) return held;

		const to = Math.round((p - base) / pitch);

		return Math.max(0, Math.min(slots.length - 1, to));
	}

	function grab(event: PointerEvent, index: number) {
		if (event.button !== 0 || !current || !grid) return;

		const node = grid;
		const id = event.pointerId;
		const startX = event.clientX;
		const startY = event.clientY;
		const far = event.pointerType === 'touch' ? THRESHOLD_TOUCH : THRESHOLD;
		const vertical = current.vertical;

		let moved = false;
		let frame = 0;
		let x = startX;
		let y = startY;

		/** Миг прошлого кадра: самоход считается временем, а не кадрами. */
		let last = 0;

		/** Прокрутка сдвинулась — места надо поправить. */
		let rolled = false;

		/** За какую точку плитки взялась рука, считая от её середины. */
		let anchor = 0;

		/*
		 * Где плитка нарисована.
		 *
		 * Рука тянет её один к одному, но место под ней переезжает: на каждом
		 * обмене оно прыгает на шаг ряда. Поэтому из хода вычитается само место
		 * — тогда в миг обмена плитка на экране стоит неподвижно, а остаток по
		 * построению не больше полушага.
		 *
		 * Прежний расчёт брал сырую дельту от точки нажатия, и без зажима плитка
		 * уезжала бы на шаг вперёд руки после каждого обмена. Зажим это прятал,
		 * заодно превращая ход в шесть точек из пятидесяти двух — ровно то, что
		 * и читалось как «рука едет, а значок стоит».
		 *
		 * Зажим остался только там, где сетка правда режет: у двух концов ряда,
		 * по её коробке. Поперёк ряда хода нет вовсе — коробка сетки по высоте
		 * равна высоте плитки, ехать некуда.
		 */
		const carry = () => {
			const at = slots[held];

			if (!at) return;

			const home = vertical ? at.y : at.x;
			const edge = vertical ? half.y : half.x;
			const low = (vertical ? frame_box.top : frame_box.left) + edge - home;
			const high = (vertical ? frame_box.bottom : frame_box.right) - edge - home;
			const want = (vertical ? y : x) - anchor - home;
			const put = Math.max(low, Math.min(high, want));

			lift = vertical ? { x: 0, y: put } : { x: put, y: 0 };
		};

		const compute = (now: number) => {
			frame = 0;

			if (!preview) return;

			// У края ряд едет сам: за девятой программой он прокручивается, и без
			// этого до неё было бы не дотянуться. Коробка сетки уже снята замером
			// — мерить её каждый кадр значило бы пересчитывать раскладку сто раз
			// в секунду
			const side = vertical
				? y > frame_box.bottom - EDGE
					? 1
					: y < frame_box.top + EDGE
						? -1
						: 0
				: x > frame_box.right - EDGE
					? 1
					: x < frame_box.left + EDGE
						? -1
						: 0;

			if (side === 0) {
				last = 0;
			} else {
				// Полсотни миллисекунд потолка: уснувшая вкладка иначе швырнула
				// бы ряд разом на всю просроченную дельту
				const span = last === 0 ? 0 : Math.min(now - last, 50);

				last = now;

				if (span > 0) {
					const by = (side * SPEED * span) / 1000;

					if (vertical) node.scrollTop += by;
					else node.scrollLeft += by;

					rolled = true;
				}
			}

			if (rolled) {
				rolled = false;

				// Одно чтение вместо девяти замеров: места сдвигаются ровно на
				// то, на сколько уехала прокрутка
				const nowX = node.scrollLeft;
				const nowY = node.scrollTop;
				const byX = nowX - scrolled.x;
				const byY = nowY - scrolled.y;

				if (byX !== 0 || byY !== 0) {
					scrolled = { x: nowX, y: nowY };

					for (const slot of slots) {
						slot.x -= byX;
						slot.y -= byY;
					}
				}
			}

			const to = rankOf((vertical ? y : x) - anchor);

			if (to !== held) {
				const next = [...preview];
				const [taken] = next.splice(held, 1);

				next.splice(to, 0, taken);

				preview = next;
				held = to;
			}

			// Ход пересчитывается и здесь, а не только на движении руки: место
			// под плиткой могло переехать — обменом или прокруткой, — пока рука
			// стояла. Обе записи уходят одним сбросом, кадра со сдвигом не бывает
			carry();

			// Пока рука стоит у края, ряд едет сам — кадры нужны и без движения
			if (side !== 0 && frame === 0) frame = requestAnimationFrame(compute);
		};

		/** Колесо во время переноса двигает ряд мимо нас — места надо поправить. */
		const rolls = () => {
			rolled = true;

			if (frame === 0) frame = requestAnimationFrame(compute);
		};

		const move = (step: PointerEvent) => {
			x = step.clientX;
			y = step.clientY;

			if (!moved) {
				// Порог только по ведущей оси: поперёк ряда плитке ехать некуда,
				// и дрожание поперёк срывало бы перенос впустую — на экране
				// ничего, зато запуск программы заглушен на триста миллисекунд
				if (Math.abs(vertical ? y - startY : x - startX) <= far) return;

				moved = true;
				held = index;
				preview = [...(current?.items ?? [])];
				measure(node);

				// За какую точку плитки взялась рука. Считается один раз: дальше
				// рука ведёт именно её, и переезды мест на это уже не влияют
				anchor = (vertical ? startY : startX) - (vertical ? slots[index].y : slots[index].x);

				/*
				 * Захват ставится на сетку, а не на плитку, и только теперь.
				 *
				 * Плитку Svelte переставляет на первом же обмене, узел проходит
				 * шаги удаления — и захват с него слетает. В окне высотой с
				 * ладонь указатель тут же оказывается снаружи: движение
				 * перестаёт приходить, отпускание не доходит вовсе, перенос
				 * залипает, и следующее нажатие уже не проходит.
				 *
				 * Не раньше порога — чтобы простое нажатие шло своим ходом и
				 * запуск программы остался запуском программы.
				 */
				node.setPointerCapture?.(id);
				node.addEventListener('scroll', rolls, { passive: true });
			}

			carry();

			// Раз на кадр: событий указателя приходит куда больше
			if (frame === 0) frame = requestAnimationFrame(compute);
		};

		const done = () => {
			cancelAnimationFrame(frame);
			frame = 0;

			// Слушатели снимаются до отпускания захвата: иначе `lostpointercapture`
			// от него же вернулся бы сюда вторым заходом
			window.removeEventListener('pointermove', move);
			window.removeEventListener('pointerup', done);
			window.removeEventListener('pointercancel', done);
			window.removeEventListener('lostpointercapture', done);
			window.removeEventListener('blur', done);
			node.removeEventListener('scroll', rolls);

			last = 0;

			if (moved) node.releasePointerCapture?.(id);

			lift = { x: 0, y: 0 };

			if (!moved) {
				// Не сдвинулись — значит это было нажатие, и оно уже своё дело
				// сделает обработчиком click
				return;
			}

			settled = performance.now();
			save();
		};

		window.addEventListener('pointermove', move);
		window.addEventListener('pointerup', done);
		// Захват отбирают, окно прячут по уходу фокуса — жест обязан закрыться сам
		window.addEventListener('pointercancel', done);
		window.addEventListener('lostpointercapture', done);
		window.addEventListener('blur', done);
	}

	async function save() {
		const next = preview;

		held = -1;

		if (!current || !next) {
			preview = null;
			return;
		}

		try {
			await groups.save({ ...current, items: next });
		} catch (e) {
			failed = String(e);
		} finally {
			// Предпросмотр держится до конца записи: иначе между сбросом и
			// приходом обновлённого списка кадр-другой показывается прежний
			// порядок, и плитка на глазах отскакивает назад
			preview = null;
		}
	}

</script>

<!-- Ключ по номеру показа: окно прячется, а не закрывается, и без пересоздания
     появление сыграло бы один раз за всю жизнь окна -->
{#key shows}
<div
	class="qrx_pop qrx_pop_open"
	style="--origin: {grow.origin}; --rise-x: {grow.x}; --rise-y: {grow.y}"
>
	<div class="qrx_pop_body" bind:this={shell}>
	{#if current}
		<!-- Шапку и подписи включает сам набор: это его вид, а не настройка окна -->
		{#if current.head || trail.length > 0}
			<header class="qrx_pop_head">
				{#if trail.length > 0}
					<button class="qrx_rec_log" onclick={back}>{i18n.t.grp_back}</button>
				{/if}

				<!-- Просим в сорока, а рисуем в двадцати четырёх: размер входит в
				     ключ хранилища, и свой, ни с кем не общий размер значил бы
				     поход в оболочку Windows на каждый показ. Сорок — тот же,
				     каким эти значки уже вытащены в главном окне и в панели -->
				<AppIcon eager want={40} path={icon(current)} fallback={current.name} size={24} />
				<span class="qrx_pop_name">{current.name}</span>

				<button
					class="qrx_pop_close"
					aria-label={i18n.t.grp_close}
					onclick={() => invoke('window_doze')}
				></button>
			</header>
		{/if}

		{#if current.items.length === 0}
			<p class="qrx_rec_task_note">{i18n.t.grp_items_none}</p>
		{:else}
				<div
						class="qrx_pop_grid"
						class:qrx_pop_grid_column={current.vertical}
						class:qrx_pop_grid_moving={held >= 0}
						bind:this={grid}
					>
					{#each rows as item, index (item.id)}
						{@const inner = item.child ? groups.list.find((each) => each.id === item.child) : null}

						<!--
							Подсказок здесь нет намеренно: окно подогнано под содержимое, и
							всплывающей подписи негде поместиться — её срезает край, и от
							неё остаётся чёрная полоска. Название и так под значком, а
							если подписи убраны, то убраны по желанию.

							Задержка появления считается от места: значки влетают друг за
							другом, а не разом. Разом — это вспышка, её глаз не читает.
						-->
						<button
							class="qrx_pop_tile"
							class:qrx_pop_tile_nested={item.child}
							class:qrx_pop_tile_held={held === index}
							data-tile={index}
							style="--at: {index}"
							title={item.tip || (item.child ? i18n.t.grp_nest : item.path)}
							onpointerdown={(event) => grab(event, index)}
							ondragstart={(event) => event.preventDefault()}
							onclick={() => {
								// Перенос уже разложил список — нажатие после него не
								// должно ещё и запускать программу. Признак по времени,
								// а не по `held`: тот к этому мигу уже сброшен
								if (performance.now() - settled < 300) return;

								if (item.child) enter(item.child);
								else launchOne(current.id, item.id);
							}}
							animate:flip={{ duration: still || held === index ? 0 : slide, easing: expoOut }}
						>
							<!-- Подъём живёт на обёртке, а не на самой плитке: переезд
							     соседей пишет плитке свой `transform` в `style`, и два
							     таких на одном узле стирали бы друг друга -->
							<span
								class="qrx_pop_tile_lift"
								style={held === index ? `--dx: ${lift.x}px; --dy: ${lift.y}px` : ''}
							>
								<AppIcon
									eager
									want={40}
									path={item.icon || (inner ? icon(inner) : item.path)}
									fallback={item.name}
									size={28}
								/>

								{#if current.labels}
									<span class="qrx_pop_tile_name">{item.name}</span>
								{/if}
							</span>
						</button>
					{/each}
				</div>

		{/if}
	{:else if ready}
		<!-- Список поднят, а набора в нём нет — значит его правда убрали -->
		<p class="qrx_rec_task_note">{i18n.t.grp_nested_gone}</p>
	{/if}

	{#if failed}
		<p class="qrx_rec_warn">{failed}</p>
	{/if}
	</div>
</div>
{/key}
