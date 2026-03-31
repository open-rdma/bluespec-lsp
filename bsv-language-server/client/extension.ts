import * as vscode from 'vscode';
import * as path from 'path';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: vscode.ExtensionContext) {
    console.log('BSV Language Server extension is now active!');
    
    // 获取配置
    const config = vscode.workspace.getConfiguration('bsv');
    const serverPath = config.get<string>('languageServer.path');
    const traceServer = config.get<string>('languageServer.trace.server') || 'off';
    const enable = config.get<boolean>('languageServer.enable', true);
    
    if (!enable) {
        console.log('BSV language server is disabled by configuration.');
        return;
    }
    
    // 确定服务器路径
    let serverModule: string;
    if (serverPath && serverPath.trim() !== '') {
        // 使用用户指定的路径
        serverModule = serverPath;
    } else {
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
    const serverOptions: ServerOptions = {
        run: {
            command: serverModule,
            args: [],
            transport: TransportKind.stdio
        },
        debug: {
            command: serverModule,
            args: ['--debug'],
            transport: TransportKind.stdio
        }
    };
    
    // 客户端选项
    const clientOptions: LanguageClientOptions = {
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
    client = new LanguageClient(
        'bsvLanguageServer',
        'BSV Language Server',
        serverOptions,
        clientOptions
    );
    
    // 设置跟踪级别
    client.setTrace(traceServer === 'verbose' ? 2 : traceServer === 'messages' ? 1 : 0);
    
    // 启动客户端
    client.start().then(() => {
        console.log('BSV Language Server client started successfully.');
        
        // 注册命令
        context.subscriptions.push(
            vscode.commands.registerCommand('bsv.restartServer', async () => {
                await client.stop();
                await client.start();
                vscode.window.showInformationMessage('BSV Language Server restarted.');
            }),
            
            vscode.commands.registerCommand('bsv.showOutput', () => {
                client.outputChannel.show();
            })
        );
    }).catch((err: any) => {
        vscode.window.showErrorMessage(`Failed to start BSV Language Server: ${err.message}`);
        console.error('Failed to start BSV Language Server:', err);
    });
    
    // 添加到订阅列表
    context.subscriptions.push(client);
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}