"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const path = __importStar(require("path"));
const node_1 = require("vscode-languageclient/node");
let client;
function activate(context) {
    console.log('BSV Language Server extension is now active!');
    // 获取配置
    const config = vscode.workspace.getConfiguration('bsv');
    const serverPath = config.get('languageServer.path');
    const traceServer = config.get('languageServer.trace.server') || 'off';
    const enable = config.get('languageServer.enable', true);
    if (!enable) {
        console.log('BSV language server is disabled by configuration.');
        return;
    }
    // 确定服务器路径
    let serverModule;
    if (serverPath && serverPath.trim() !== '') {
        // 使用用户指定的路径
        serverModule = serverPath;
    }
    else {
        // 使用默认路径（相对路径）
        serverModule = context.asAbsolutePath(path.join('..', 'target', 'release', 'bsv-language-server'));
    }
    console.log(`Using server module: ${serverModule}`);
    // 如果服务器模块不存在，尝试从系统PATH查找
    const fs = require('fs');
    if (!fs.existsSync(serverModule)) {
        serverModule = 'bsv-language-server'; // 回退到系统PATH
    }
    // 服务器选项
    const serverOptions = {
        run: {
            command: serverModule,
            args: [],
            transport: node_1.TransportKind.stdio
        },
        debug: {
            command: serverModule,
            args: ['--debug'],
            transport: node_1.TransportKind.stdio
        }
    };
    // 客户端选项
    const clientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'bsv' },
            { scheme: 'untitled', language: 'bsv' }
        ],
        synchronize: {
            // 同步配置更改
            configurationSection: 'bsv',
            // 通知服务器文件更改
            fileEvents: [
                vscode.workspace.createFileSystemWatcher('**/*.bsv'),
                vscode.workspace.createFileSystemWatcher('**/*.bs')
            ]
        },
        outputChannel: vscode.window.createOutputChannel('BSV Language Server'),
        traceOutputChannel: vscode.window.createOutputChannel('BSV Language Server Trace'),
        initializationOptions: {
            // 传递给服务器的初始化选项
            workspaceFolders: vscode.workspace.workspaceFolders ?
                vscode.workspace.workspaceFolders.map(folder => folder.uri.toString()) : []
        }
    };
    // 创建语言客户端
    client = new node_1.LanguageClient('bsvLanguageServer', 'BSV Language Server', serverOptions, clientOptions);
    // 设置跟踪级别
    client.setTrace(traceServer === 'verbose' ? 2 : traceServer === 'messages' ? 1 : 0);
    // 启动客户端
    client.start().then(() => {
        console.log('BSV Language Server client started successfully.');
        // 注册命令
        context.subscriptions.push(vscode.commands.registerCommand('bsv.restartServer', async () => {
            await client.stop();
            await client.start();
            vscode.window.showInformationMessage('BSV Language Server restarted.');
        }), vscode.commands.registerCommand('bsv.showOutput', () => {
            client.outputChannel.show();
        }));
    }).catch((err) => {
        vscode.window.showErrorMessage(`Failed to start BSV Language Server: ${err.message}`);
        console.error('Failed to start BSV Language Server:', err);
    });
    // 添加到订阅列表
    context.subscriptions.push(client);
}
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
//# sourceMappingURL=extension.js.map