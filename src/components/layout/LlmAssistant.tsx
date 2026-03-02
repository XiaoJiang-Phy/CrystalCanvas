// [Overview: LLM Assistant chat panel with provider configuration and command preview]
// Copyright (c) 2026 Xiao Jiang and CrystalCanvas Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0
import React, { useState, useRef, useEffect } from 'react';
import { cn } from '../../utils/cn';
import { Bot, Settings2, Send, X, Loader2, Play } from '../../utils/Icons';
import { safeInvoke } from '../../utils/tauri-mock';

interface LlmAssistantProps {
    isOpen: boolean;
    onClose: () => void;
}

interface Message {
    id: string;
    role: 'user' | 'assistant' | 'system';
    text: string;
    commandJson?: string;
}

export const LlmAssistant: React.FC<LlmAssistantProps> = ({ isOpen, onClose }) => {
    const [messages, setMessages] = useState<Message[]>([]);
    const [input, setInput] = useState('');
    const [loading, setLoading] = useState(false);

    // Settings state
    const [showSettings, setShowSettings] = useState(true);
    const [providerType, setProviderType] = useState('openai');
    const [apiKey, setApiKey] = useState('');
    const [model, setModel] = useState('o4-mini');

    const messagesEndRef = useRef<HTMLDivElement>(null);

    // Default models based on M9 configuration
    const providerDefaults = {
        openai: 'o4-mini',
        deepseek: 'deepseek-r2',
        claude: 'claude-4-sonnet-20260228',
        gemini: 'gemini-3.0-flash',
        ollama: 'qwen3.5-math'
    };

    const handleProviderChange = (newProvider: string) => {
        setProviderType(newProvider);
        setModel(providerDefaults[newProvider as keyof typeof providerDefaults]);
    };

    const saveSettings = async () => {
        try {
            await safeInvoke('llm_configure', { providerType, apiKey, model });
            setShowSettings(false);
            if (messages.length === 0) {
                setMessages([{
                    id: Date.now().toString(),
                    role: 'system',
                    text: 'Assistant configured. Wait for operations or type your request.'
                }]);
            }
        } catch (e) {
            console.error('Failed to configure LLM:', e);
            alert('Failed to configure LLM');
        }
    };

    const handleSend = async () => {
        if (!input.trim() || loading) return;

        const userMsg: Message = { id: Date.now().toString(), role: 'user', text: input.trim() };
        setMessages(prev => [...prev, userMsg]);
        setInput('');
        setLoading(true);

        try {
            const rawResponse = await safeInvoke<string>('llm_chat', { userMessage: userMsg.text });
            const response = rawResponse || '';

            let cleanResponse = response;
            let commandJson: string | undefined = undefined;

            try {
                // Check if response is pure JSON mapping to Schema
                const parsed = JSON.parse(response);
                if (parsed.action) {
                    commandJson = response;
                    cleanResponse = `I have prepared the action: ${parsed.action}`;
                }
            } catch {
                // Look for markdown JSON blocks if LLM ignored strict instructions
                const match = response.match(/```(?:json)?\s*(\{[\s\S]*?\})\s*```/);
                if (match) {
                    commandJson = match[1];
                    try {
                        JSON.parse(commandJson); // Validate
                        cleanResponse = response.replace(/```(?:json)?\s*(\{[\s\S]*?\})\s*```/, "\n[Command Extracted]\n");
                    } catch {
                        commandJson = undefined;
                    }
                }
            }

            const botMsg: Message = {
                id: Date.now().toString(),
                role: 'assistant',
                text: cleanResponse,
                commandJson
            };
            setMessages(prev => [...prev, botMsg]);

        } catch (e: unknown) {
            setMessages(prev => [...prev, { id: Date.now().toString(), role: 'system', text: `Error: ${String(e)}` }]);
        } finally {
            setLoading(false);
        }
    };

    const handleExecute = async (json: string) => {
        try {
            await safeInvoke('llm_execute_command', { commandJson: json });
            setMessages(prev => [...prev, { id: Date.now().toString(), role: 'system', text: 'Command executed successfully in the sandbox.' }]);
        } catch (e: unknown) {
            setMessages(prev => [...prev, { id: Date.now().toString(), role: 'system', text: `Execution failed: ${String(e)}` }]);
        }
    };

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    if (!isOpen) return null;

    return (
        <div className={cn(
            "absolute bottom-10 right-[250px] w-80 z-30",
            "bg-white/95 dark:bg-slate-900/95 backdrop-blur-xl",
            "border border-white/30 dark:border-slate-700/50",
            "rounded-xl shadow-2xl flex flex-col pointer-events-auto",
            "animate-in slide-in-from-bottom-2",
            "transition-all duration-300",
            showSettings ? "max-h-[400px]" : "max-h-[500px]"
        )}>
            {/* Header */}
            <div className="px-3 py-2.5 flex justify-between items-center bg-gradient-to-r from-emerald-500/10 to-transparent border-b border-slate-100 dark:border-slate-800">
                <div className="flex items-center gap-2">
                    <Bot className="w-4 h-4 text-emerald-600 dark:text-emerald-400" />
                    <h3 className="font-semibold text-sm text-slate-800 dark:text-slate-200">LLM Assistant</h3>
                </div>
                <div className="flex items-center gap-2">
                    <button
                        onClick={() => setShowSettings(!showSettings)}
                        className={cn("transition-colors", showSettings ? "text-emerald-500" : "text-slate-400 hover:text-emerald-500")}
                        title="Settings"
                    >
                        <Settings2 className="w-4 h-4" />
                    </button>
                    <button
                        onClick={onClose}
                        className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 transition-colors"
                        title="Close"
                    >
                        <X className="w-4 h-4" />
                    </button>
                </div>
            </div>

            {/* Content Area */}
            <div className="flex-1 overflow-hidden flex flex-col w-full h-[400px]">
                {showSettings ? (
                    <div className="p-4 flex flex-col gap-3 overflow-y-auto">
                        <div className="flex flex-col gap-1">
                            <label className="text-xs font-medium text-slate-600 dark:text-slate-300">Provider</label>
                            <select
                                value={providerType}
                                onChange={(e) => handleProviderChange(e.target.value)}
                                className="w-full text-sm bg-slate-50 dark:bg-slate-800 border-slate-200 dark:border-slate-700 rounded-md py-1.5 px-2 text-slate-700 dark:text-slate-200 outline-none"
                            >
                                <option value="openai">OpenAI Compatible</option>
                                <option value="deepseek">DeepSeek</option>
                                <option value="claude">Claude (Anthropic)</option>
                                <option value="gemini">Gemini (Google)</option>
                                <option value="ollama">Ollama (Local)</option>
                            </select>
                        </div>

                        {providerType !== 'ollama' && (
                            <div className="flex flex-col gap-1">
                                <label className="text-xs font-medium text-slate-600 dark:text-slate-300">API Key</label>
                                <input
                                    type="password"
                                    value={apiKey}
                                    onChange={(e) => setApiKey(e.target.value)}
                                    placeholder="Enter API Key"
                                    className="w-full text-sm bg-slate-50 dark:bg-slate-800 border-slate-200 dark:border-slate-700 rounded-md py-1.5 px-2 text-slate-700 dark:text-slate-200 outline-none focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500/30"
                                />
                            </div>
                        )}

                        <div className="flex flex-col gap-1">
                            <label className="text-xs font-medium text-slate-600 dark:text-slate-300">Model</label>
                            <input
                                type="text"
                                value={model}
                                onChange={(e) => setModel(e.target.value)}
                                className="w-full text-sm bg-slate-50 dark:bg-slate-800 border-slate-200 dark:border-slate-700 rounded-md py-1.5 px-2 text-slate-700 dark:text-slate-200 outline-none focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500/30"
                            />
                        </div>

                        <button
                            onClick={saveSettings}
                            className="mt-2 w-full py-2 bg-emerald-500 hover:bg-emerald-600 text-white rounded-md text-sm font-medium transition-colors"
                        >
                            Save & Chat
                        </button>
                    </div>
                ) : (
                    <>
                        {/* Chat History */}
                        <div className="flex-1 overflow-y-auto p-3 flex flex-col gap-3 text-sm">
                            {messages.map(msg => (
                                <div key={msg.id} className={cn("flex flex-col max-w-[90%]",
                                    msg.role === 'user' ? "items-end self-end" :
                                        msg.role === 'system' ? "items-center self-center" : "items-start self-start"
                                )}>
                                    <div className={cn("px-3 py-2 rounded-lg",
                                        msg.role === 'user' ? "bg-emerald-500 text-white" :
                                            msg.role === 'system' ? "bg-amber-100 dark:bg-amber-900/30 text-amber-800 dark:text-amber-200 text-[11px] text-center" :
                                                "bg-slate-100 dark:bg-slate-800 text-slate-800 dark:text-slate-200"
                                    )}>
                                        {msg.text}
                                    </div>
                                    {msg.commandJson && (
                                        <div className="mt-1 flex flex-col gap-1 w-full min-w-[200px] bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded p-2">
                                            <pre className="text-[10px] text-slate-600 dark:text-slate-400 overflow-x-auto">
                                                {msg.commandJson}
                                            </pre>
                                            <button
                                                onClick={() => handleExecute(msg.commandJson!)}
                                                className="self-end mt-1 flex items-center gap-1 bg-emerald-100 dark:bg-emerald-500/20 text-emerald-700 dark:text-emerald-400 py-1 px-2 rounded text-xs font-medium hover:bg-emerald-200 dark:hover:bg-emerald-500/30 transition-colors"
                                            >
                                                <Play className="w-3 h-3 fill-current" /> Execute
                                            </button>
                                        </div>
                                    )}
                                </div>
                            ))}
                            {loading && (
                                <div className="self-start text-slate-400 flex items-center gap-2 text-sm p-2">
                                    <Loader2 className="w-4 h-4 animate-spin" /> Thinking...
                                </div>
                            )}
                            <div ref={messagesEndRef} />
                        </div>

                        {/* Input Area */}
                        <div className="p-2 border-t border-slate-100 dark:border-slate-800 bg-white dark:bg-slate-900 flex gap-2">
                            <textarea
                                value={input}
                                onChange={(e) => setInput(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === 'Enter' && !e.shiftKey) {
                                        e.preventDefault();
                                        handleSend();
                                    }
                                }}
                                className={cn(
                                    "flex-1 h-10 min-h-[40px] max-h-32 bg-slate-50 dark:bg-slate-800/50 rounded-md",
                                    "border border-slate-200 dark:border-slate-700",
                                    "py-2.5 px-3 text-xs text-slate-700 dark:text-slate-300",
                                    "outline-none resize-none",
                                    "focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500/30",
                                    "transition-all placeholder:text-slate-400"
                                )}
                                placeholder="Type your instruction..."
                            />
                            <button
                                onClick={handleSend}
                                disabled={loading || !input.trim()}
                                className="h-10 w-10 shrink-0 bg-emerald-500 hover:bg-emerald-600 disabled:opacity-50 disabled:hover:bg-emerald-500 text-white rounded-md flex items-center justify-center transition-colors shadow-sm"
                            >
                                <Send className="w-4 h-4" />
                            </button>
                        </div>
                    </>
                )}
            </div>
        </div>
    );
};
