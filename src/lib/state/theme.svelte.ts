/**
 * Тема оформления.
 *
 * Исходное приложение переключало тему, создавая `<link href="/src/lib/App/style/
 * themes/theme_black.css">` прямо в рантайме. Такой путь существует только у
 * dev-сервера, поэтому в собранном приложении тема не применялась вовсе.
 *
 * Здесь оба набора правил лежат в общем бандле, отделённые селектором
 * `:root[data-theme=...]`, а переключение — это смена одного атрибута.
 */

/** Значения, которые пишет в localStorage экран настроек. */
export type ThemeChoice = 'Black' | 'White' | 'Systemic';

/** Что в итоге проставляется в data-theme. */
export type Resolved = 'dark' | 'light';

const STORAGE_KEY = 'PlayerUserTheme';

const SYSTEM_DARK = '(prefers-color-scheme: dark)';

class Theme {
	/** Выбор пользователя, включая «как в системе». */
	choice = $state<ThemeChoice>('Black');

	/** Тема, которая реально применена сейчас. */
	resolved = $state<Resolved>('dark');

	/** Поднимает сохранённый выбор и вешает слежение за системной темой. */
	restore(): () => void {
		const saved = localStorage.getItem(STORAGE_KEY) as ThemeChoice | null;
		this.choice = saved ?? 'Black';
		this.apply();

		const query = window.matchMedia(SYSTEM_DARK);
		const onChange = () => {
			if (this.choice === 'Systemic') this.apply();
		};

		query.addEventListener('change', onChange);
		return () => query.removeEventListener('change', onChange);
	}

	/** Ставит тему и запоминает выбор. */
	set(choice: ThemeChoice) {
		this.choice = choice;
		localStorage.setItem(STORAGE_KEY, choice);
		this.apply();
	}

	private apply() {
		const systemDark = window.matchMedia(SYSTEM_DARK).matches;

		this.resolved =
			this.choice === 'White' ? 'light' : this.choice === 'Systemic' && !systemDark ? 'light' : 'dark';

		document.documentElement.dataset.theme = this.resolved;
	}
}

export const theme = new Theme();
