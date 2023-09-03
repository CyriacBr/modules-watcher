import * as B from "./b.js";
import { FILE_1 } from "./file1.js";
import file2 from "./file2.js";
import "./file3.js";
import { FILE_4 } from "./file4";
import { FILE_4_SOMETHING } from "./file4.something";
import { FILE_5 } from "./file5";
import { D } from "./d";

export * from './e.js';

import("./file6");
require("./file7");

import * as tsNode from 'ts-node';

import './file8.css';
import './file10.scss';

import { FILE_13 } from "~/file13.js";
