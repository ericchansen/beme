/* @refresh reload */
import { render } from "solid-js/web";
import ControlBar from "./ControlBar";
import "../app.css";

render(() => <ControlBar />, document.getElementById("root") as HTMLElement);
