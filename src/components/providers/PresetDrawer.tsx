import React, { useState } from 'react';
import { useAppStore } from '@/store/useAppStore';
import { Preset } from '@/types';
import { X, Search, Sparkles, Download, Check } from 'lucide-react';

export const PresetDrawer: React.FC = () => {
  const { isPresetDrawerOpen, closePresetDrawer, presets, importPreset, providers } =
    useAppStore();

  const [search, setSearch] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string>('All');
  const [importingId, setImportingId] = useState<string | null>(null);
  const [importedIds, setImportedIds] = useState<Set<string>>(new Set());

  if (!isPresetDrawerOpen) return null;

  const categories = ['All', 'Commercial', 'Aggregator', 'Cloud', 'Open Source / Local'];

  const filteredPresets = presets.filter((p) => {
    const matchesSearch =
      p.name.toLowerCase().includes(search.toLowerCase()) ||
      p.description.toLowerCase().includes(search.toLowerCase()) ||
      p.defaultModels.some((m) => m.toLowerCase().includes(search.toLowerCase()));

    const matchesCat = selectedCategory === 'All' || p.category === selectedCategory;
    return matchesSearch && matchesCat;
  });

  const handleImport = async (preset: Preset) => {
    setImportingId(preset.id);
    try {
      await importPreset(preset.id);
      setImportedIds((prev) => new Set([...prev, preset.id]));
      setTimeout(() => {
        setImportedIds((prev) => {
          const next = new Set(prev);
          next.delete(preset.id);
          return next;
        });
      }, 2500);
    } finally {
      setImportingId(null);
    }
  };

  const isAlreadyAdded = (presetId: string) => {
    return providers.some((p) => p.id === presetId);
  };

  return (
    <div className="fixed inset-0 z-50 flex justify-end bg-black/50 backdrop-blur-sm">
      <div className="w-full max-w-2xl bg-white dark:bg-gray-900 h-full border-l border-gray-200 dark:border-gray-800 flex flex-col shadow-2xl animate-in slide-in-from-right duration-200">
        {/* Drawer Header */}
        <div className="px-6 py-4 border-b border-gray-200 dark:border-gray-800 flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-lg bg-indigo-50 dark:bg-indigo-950/60 text-indigo-600 dark:text-indigo-400 flex items-center justify-center border border-indigo-200/50 dark:border-indigo-800/40">
              <Sparkles className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-base font-semibold text-gray-900 dark:text-gray-100">
                Preset Providers Store (30+)
              </h2>
              <p className="text-xs text-gray-500 dark:text-gray-400">
                Instantly import pre-configured API endpoints and models
              </p>
            </div>
          </div>
          <button
            onClick={closePresetDrawer}
            className="p-1 rounded-lg text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Search & Category Filter */}
        <div className="p-4 border-b border-gray-200 dark:border-gray-800 space-y-3 bg-gray-50/50 dark:bg-gray-900/40">
          <div className="relative">
            <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
            <input
              type="text"
              placeholder="Search by vendor, model (e.g. Claude, DeepSeek, Qwen)..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full pl-9 pr-4 py-1.5 text-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder-gray-400 focus:outline-none focus:ring-2 focus:ring-indigo-500/20"
            />
          </div>

          <div className="flex flex-wrap gap-1.5">
            {categories.map((cat) => (
              <button
                key={cat}
                onClick={() => setSelectedCategory(cat)}
                className={`px-2.5 py-1 text-xs rounded-md font-medium transition-colors ${
                  selectedCategory === cat
                    ? 'bg-indigo-600 text-white shadow-sm'
                    : 'bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300 border border-gray-200 dark:border-gray-700 hover:bg-gray-100 dark:hover:bg-gray-700/50'
                }`}
              >
                {cat}
              </button>
            ))}
          </div>
        </div>

        {/* Preset Cards List */}
        <div className="flex-1 overflow-y-auto p-4 space-y-3">
          {filteredPresets.map((preset) => {
            const added = isAlreadyAdded(preset.id);
            const isImportedJustNow = importedIds.has(preset.id);
            const isImporting = importingId === preset.id;

            return (
              <div
                key={preset.id}
                className="border border-gray-200 dark:border-gray-800 rounded-xl p-4 bg-white dark:bg-gray-900 hover:border-indigo-400 dark:hover:border-indigo-700 transition-all flex flex-col justify-between"
              >
                <div>
                  <div className="flex items-start justify-between gap-3 mb-2">
                    <div>
                      <div className="flex items-center gap-2">
                        <h4 className="text-sm font-semibold text-gray-900 dark:text-gray-100">
                          {preset.name}
                        </h4>
                        <span className="text-[10px] uppercase font-mono px-1.5 py-0.2 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 font-medium">
                          {preset.protocol}
                        </span>
                        <span className="text-[10px] px-1.5 py-0.2 rounded bg-indigo-50 dark:bg-indigo-950/40 text-indigo-600 dark:text-indigo-400">
                          {preset.category}
                        </span>
                      </div>
                      <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                        {preset.description}
                      </p>
                    </div>

                    <button
                      onClick={() => handleImport(preset)}
                      disabled={isImporting}
                      className={`flex items-center gap-1 px-3 py-1.5 text-xs font-medium rounded-lg transition-all shadow-sm ${
                        isImportedJustNow
                          ? 'bg-emerald-600 text-white'
                          : added
                          ? 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
                          : 'bg-indigo-600 text-white hover:bg-indigo-700'
                      }`}
                    >
                      {isImportedJustNow ? (
                        <>
                          <Check className="w-3.5 h-3.5" />
                          <span>Imported!</span>
                        </>
                      ) : isImporting ? (
                        <span>Importing...</span>
                      ) : added ? (
                        <>
                          <Download className="w-3.5 h-3.5" />
                          <span>Re-import</span>
                        </>
                      ) : (
                        <>
                          <Download className="w-3.5 h-3.5" />
                          <span>Import</span>
                        </>
                      )}
                    </button>
                  </div>

                  <div className="text-[11px] font-mono text-gray-500 dark:text-gray-400 truncate mb-2">
                    {preset.baseUrl}
                  </div>

                  <div className="flex flex-wrap gap-1">
                    {preset.defaultModels.map((m) => (
                      <span
                        key={m}
                        className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-gray-50 dark:bg-gray-800/80 border border-gray-100 dark:border-gray-800 text-gray-600 dark:text-gray-400"
                      >
                        {m}
                      </span>
                    ))}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
};
