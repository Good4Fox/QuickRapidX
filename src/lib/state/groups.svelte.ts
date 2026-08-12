import { invoke } from '@tauri-apps/api/core';

/**
 * Наборы ярлыков.
 *
 * Один и тот же список нужен двум окнам — главному, где наборы собирают, и
 * панели трея, откуда их открывают. Поэтому состояние общее, а не своё у
 * каждого экрана: иначе собранный набор появлялся бы в трее только после
 * перезапуска.
 *
 * Окна при этом разные, и памяти у них тоже разные: панель трея перечитывает
 * список каждый раз, когда её открывают, — файл к тому времени мог измениться
 * в главном окне.
 */
export type Shortcut = {
	id: string;
	name: string;
	/** Что открывать: программа, ярлык, папка или ссылка. */
	path: string;
	args: string;
	admin: boolean;
	isolated: boolean;
	/** Своя подсказка при наведении. Пусто — показывается путь. */
	tip: string;
	/** Свой значок записи. Пусто — берётся из самого файла. */
	icon: string;
	/** Вложенный набор вместо программы: идентификатор другого набора. */
	child: string;
};

export type Group = {
	id: string;
	name: string;
	icon: string;
	/** Своя иконка в системном трее. */
	tray: boolean;
	/** Показывать в общей панели у основного значка. */
	panel: boolean;
	/** Показывать шапку в окне набора: значок, название и крестик. */
	head: boolean;
	/** Подписывать программы в окне набора. */
	labels: boolean;
	/** Ставить программы столбцом, а не строкой. */
	vertical: boolean;
	items: Shortcut[];
};

type Started = { ok: boolean; count: number; message: string };

class Groups {
	list = $state<Group[]>([]);
	loading = $state(false);

	/**
	 * Какой набор открыт в главном окне. Пусто — показывается список.
	 *
	 * Живёт здесь, а не внутри экрана, ровно по одной причине: закрывать набор
	 * должен уметь и тот, кто снаружи. Повторное нажатие на «Наборы» в левом
	 * ряду — это и есть «назад», и добираться до кнопки в углу для этого не
	 * нужно.
	 */
	open = $state('');

	private asked = false;

	/** Читает список один раз за жизнь окна. */
	async ensure() {
		if (this.asked) return;

		this.asked = true;
		await this.reload();
	}

	async reload() {
		this.loading = true;

		try {
			this.list = await invoke<Group[]>('group_list');
		} catch (e) {
			console.error('наборы не прочитаны:', e);
		}

		this.loading = false;
	}

	async save(group: Group) {
		await invoke('group_save', { group: snapshot(group) });
		await this.reload();
	}

	async remove(id: string) {
		await invoke('group_remove', { id });
		await this.reload();
	}

	async reorder(ids: string[]) {
		await invoke('group_reorder', { ids });
		await this.reload();
	}

	launch(group: string, item: string): Promise<Started> {
		return invoke<Started>('group_launch', { group, item });
	}

	launchAll(id: string): Promise<Started> {
		return invoke<Started>('group_launch_all', { id });
	}

	/** Наборы для общей панели у основного значка. */
	get inPanel(): Group[] {
		return this.list.filter((group) => group.panel);
	}
}

/** Пустой набор с новым идентификатором. */
export function blankGroup(name: string): Group {
	return {
		id: crypto.randomUUID(),
		name,
		icon: '',
		tray: true,
		panel: true,
		head: true,
		labels: true,
		vertical: false,
		items: []
	};
}

/** Запись из выбранного пути. Название берётся из имени файла. */
export function shortcutFor(path: string): Shortcut {
	const tail = path.split(/[\\/]/).pop() ?? path;
	const name = tail.replace(/\.(exe|lnk|url|bat|cmd|msi)$/i, '');

	return {
		id: crypto.randomUUID(),
		name: name || path,
		path,
		args: '',
		admin: false,
		isolated: false,
		tip: '',
		icon: '',
		child: ''
	};
}

/** Запись-вложение: ведёт в другой набор, а не в программу. */
export function nestedFor(group: Group): Shortcut {
	return {
		id: crypto.randomUUID(),
		name: group.name,
		path: '',
		args: '',
		admin: false,
		isolated: false,
		tip: '',
		icon: '',
		child: group.id
	};
}

/** Руны отдают обёртку; в ядро должен уехать обычный объект. */
function snapshot(group: Group): Group {
	return {
		...group,
		items: group.items.map((item) => ({ ...item }))
	};
}

export const groups = new Groups();
