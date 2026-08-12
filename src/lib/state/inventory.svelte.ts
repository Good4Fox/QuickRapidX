import { invoke } from '@tauri-apps/api/core';

/**
 * Осмотр машины.
 *
 * Заменяет прежний поиск установленного. Тот знал один источник — реестр
 * удаления — и объявлял программой каждую подпапку в четырёх корнях, включая
 * `Temp` и кэш пакетных менеджеров.
 *
 * Список держится здесь, а не в экране: осмотр идёт секунды, и повторять его
 * при каждом переходе между разделами незачем.
 */
export type Kind = 'installed' | 'store' | 'tool' | 'portable';

export type Finding = {
	key: string;
	name: string;
	kind: Kind;
	version: string;
	publisher: string;
	location: string;
	/** Откуда брать значок. Может быть `.ico` — реестр хранит здесь значок. */
	exe: string;
	/** Что запускать. Пусто — запускать нечего. */
	launch: string;
	size: number;
	uninstall: string;
};

class Inventory {
	findings = $state<Finding[]>([]);
	scanning = $state(false);

	/** Когда осматривали в последний раз. Пусто — ещё ни разу. */
	scannedAt = $state<Date | null>(null);

	/** Осматривает машину. Повторный вызов во время осмотра ничего не делает. */
	async scan() {
		if (this.scanning) return;
		this.scanning = true;

		try {
			this.findings = await invoke<Finding[]>('inventory_scan');
			this.scannedAt = new Date();
		} catch (e) {
			console.error('осмотр не удался:', e);
		}

		this.scanning = false;
	}

	/** Осматривает, если ещё не осматривали. */
	async ensure() {
		if (this.findings.length === 0 && !this.scanning) await this.scan();
	}

	count(kind: Kind): number {
		return this.findings.filter((finding) => finding.kind === kind).length;
	}
}

export const inventory = new Inventory();
