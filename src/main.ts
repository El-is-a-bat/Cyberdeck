import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";


interface App {
    name: string;
    comment: string;
    icon: string;
    exec: string;
}

interface SlayfiConfig {
    apps_per_page: number
}
// TODO remove?
let apps: App[] = [];
// TODO change to list?
let appsEntries: HTMLDivElement[] = [];
let availableApps: HTMLDivElement[] = [];
let config: SlayfiConfig;
let maxPages: number;
let currentPage = 0;
let currentSelectedIdx = 0;
const filter = document.getElementById("filter") as HTMLInputElement;
const container = document.getElementById("app-list") as HTMLDivElement;

async function fetchApps() {
    // apps = await invoke("list_desktop_applications");
    apps = await invoke("try_get_cached_applications");
}

async function createAppsEntries() {
    container.addEventListener("click", (e) => {
        let clickedItem = e.target as HTMLDivElement;
        let entry = clickedItem.closest(".entry") as HTMLDivElement;
        if (entry) {
            selectApp(entry);
        } else {
            console.log("somehow clicked item is not in entry");
        }
    });
    container.addEventListener("dblclick", () => {
        //const appName = entry.querySelector(".app-name")?.textContent;
        //if (appName) {
        //    runApp(appName);
        //}
    });

    const isDev = await invoke<boolean>("is_dev");

    apps.forEach((app: App, index: number) => {
        const entry = document.createElement("div");
        entry.className = "entry";
        entry.id = app.name;

        appsEntries.push(entry);

        // smth like rarity in cyberpunk
        // TODO later
        const appType = document.createElement("div");
        appType.className = "app-type";
        entry.appendChild(appType);

        const appInfo = document.createElement("div");
        appInfo.className = "app-info";
        entry.appendChild(appInfo);

        const appIcon = document.createElement("img");
        appIcon.src = convertFileSrc(app.icon);
        appIcon.className = "app-icon";
        appInfo.appendChild(appIcon);

        if (isDev) {
            const elementIdx = document.createElement("div");
            elementIdx.textContent = `[${index}] `;
            appInfo.appendChild(elementIdx);
        }

        const appName = document.createElement("div");
        appName.className = "app-name";
        appName.textContent = app.name;
        appInfo.appendChild(appName);
    })
}

function selectAppByIdx(idx: number) {
    availableApps[currentSelectedIdx].classList.remove("selected");
    availableApps[idx].classList.add("selected");
    currentSelectedIdx = idx;
}

function selectApp(element: HTMLDivElement) {
    selectAppByIdx(availableApps.indexOf(element));
}

function setPage(page: number) {
    currentPage = page;

    while (container.firstElementChild) {
        container.removeChild(container.lastElementChild!);
    }

    if (availableApps.length === 0) {
        console.log("No available apps after filter");
        return;
    }

    // TODO research for more elegant way
    for (let i = 0, idx; i < config.apps_per_page; i++) {
        idx = page * config.apps_per_page + i;
        if (idx < availableApps.length) {
            container.appendChild(availableApps[idx]);
        }
    }
}

function nextPage() {
    setPage((currentPage + 1) % maxPages);
}

function prevPage() {
    setPage((currentPage - 1 + maxPages) % maxPages);
}

async function addAppSelection() {
    document.addEventListener("keydown", (e) => {

        const selected = document.querySelector(".entry.selected") as HTMLDivElement;
        if (selected === null) {
            console.log("Selected item is missing");
            return;
        }

        let newSelectedIndex = currentSelectedIdx;

        //TODO handle if last page less than apps_per_page
        // if page=0 and selectedIdx=0 and ArrowLeft, selected item outside page.
        // if scroll by ArrowRight selected item also moves itself from existence

        // outside of switch because of block scope
        switch (e.key) {
            case "ArrowUp":
                e.preventDefault();
                newSelectedIndex -= 1;
                if (newSelectedIndex < currentPage * config.apps_per_page) {
                    prevPage();
                }
                if (newSelectedIndex < 0) {
                    newSelectedIndex = availableApps.length - 1;
                }
                selectAppByIdx(newSelectedIndex);
                break;
            case "ArrowDown":
                e.preventDefault();
                newSelectedIndex += 1;
                let lastIdxOnPage = currentPage * config.apps_per_page + config.apps_per_page - 1;
                if (newSelectedIndex > lastIdxOnPage || newSelectedIndex >= availableApps.length) {
                    nextPage();
                }
                if (newSelectedIndex >= availableApps.length) {
                    newSelectedIndex = 0;
                }
                selectAppByIdx(newSelectedIndex);
                break;
            case "ArrowLeft":
                e.preventDefault();
                newSelectedIndex = currentSelectedIdx - config.apps_per_page;
                if (newSelectedIndex < 0) {
                    newSelectedIndex = (maxPages - 1) * config.apps_per_page + currentSelectedIdx;
                    if (newSelectedIndex >= availableApps.length) {
                        newSelectedIndex = availableApps.length - 1;
                    }
                }
                prevPage();
                selectAppByIdx(newSelectedIndex);
                break;
            case "ArrowRight":
                e.preventDefault();
                newSelectedIndex = currentSelectedIdx + config.apps_per_page;
                // used to determine if we need to shift index when moving to first page
                const isEvenlyDivisible = availableApps.length % config.apps_per_page === 0;
                if (newSelectedIndex > availableApps.length) {
                    if (currentPage === maxPages - 2) {
                        newSelectedIndex = availableApps.length - 1
                    }
                    if (currentPage === maxPages - 1) {
                        newSelectedIndex = newSelectedIndex % (maxPages * config.apps_per_page);
                    }
                }
                if (newSelectedIndex === availableApps.length) {
                    // Like if apps.lenght = 90, apps_per_page = 5 and currentSelectedIdx = 85, then newSelectedIndex = 0
                    // because instead new selected item would be on last page, but we move to first page
                    if (isEvenlyDivisible) {
                        newSelectedIndex = 0;
                    } else {
                        newSelectedIndex = availableApps.length - (isEvenlyDivisible ? 0 : 1);
                    }
                }
                selectAppByIdx(newSelectedIndex);
                nextPage();
                break;
            case "Enter":
                const appName = selected.querySelector(".app-name")?.textContent;
                if (appName) {
                    runApp(appName);
                }
                break;
        }
    })

}

function filterApps() {
    // TODO highlight the entered charackters 
    // TODO search regardless onf the input language
    let filterText = filter.value.toLowerCase();

    availableApps.length = 0;

    let app, appName;
    appsEntries.forEach(entry => {
        app = entry.querySelector(".app-name") as HTMLDivElement;
        appName = app.textContent || app.innerText;
        if (appName.toLowerCase().indexOf(filterText) > -1) {
            availableApps.push(entry.cloneNode(true) as HTMLDivElement);
        }
    });

    maxPages = Math.ceil(availableApps.length / config.apps_per_page);
    setPage(0);
    selectAppByIdx(0);
}

function runApp(appName: string) {
    let app = apps.find(app => app.name == appName);
    if (app) {
        console.log("Running {} with command: {}", appName, app.exec);
        invoke("start_program", { "exec": app.exec });
    }
}

window.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
        invoke("exit").then(() => console.log("Exiting app"));
    }
});

// disable arrow key actions for the filter input
filter.addEventListener("keydown", (e) => {
    if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        e.preventDefault();
    }
});

async function main() {
    await invoke<string>("get_config").then((raw) => {
        config = JSON.parse(raw) as SlayfiConfig;
        console.log("Config loaded: ", config);
    });

    await fetchApps();

    if (apps.length === 0) {
        // TODO show that apps not found
        console.log("Apps not found");
        return;
    }

    maxPages = Math.ceil(apps.length / config.apps_per_page);

    await createAppsEntries();

    if (appsEntries.length == 0) {
        console.log("Apps entries lenght is 0, smth went wrong(");
        return;
    }

    availableApps = appsEntries.map(entry => entry.cloneNode(true) as HTMLDivElement);
    availableApps[0].classList.add("selected");
    setPage(0);

    const filter = document.getElementById("filter") as HTMLInputElement;
    filter.focus();
    filter.oninput = filterApps;

    console.log(apps);
    await addAppSelection();
}

main();
