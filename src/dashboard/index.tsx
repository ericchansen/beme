/* @refresh reload */
import { render } from "solid-js/web";
import Dashboard from "./Dashboard";
import "../app.css";

render(() => <Dashboard />, document.getElementById("root") as HTMLElement);
