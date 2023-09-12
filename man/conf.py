# Configuration file for the Sphinx documentation builder.
#
# For the full list of built-in configuration values, see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html

# -- Project information -----------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#project-information

project = 'SIMBIOTA Client'
copyright = '2023, Ukatemi Technologies .'
author = 'Ukatemi Technologies Zrt.'
release = '0.0.3'

# -- General configuration ---------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#general-configuration

# Needed for Markdown support
extensions = ['myst_parser']

exclude_patterns = []

language = 'en'

# -- Options for HTML output -------------------------------------------------
# https://www.sphinx-doc.org/en/master/usage/configuration.html#options-for-html-output

# -- Other configuration ---------------------------------------------------
source_suffix = {
    '.rst': 'restructuredtext',
    '.md': 'markdown',
}

myst_heading_anchors = 4

html_show_sphinx = False

html_favicon = "favicon.ico"

man_make_section_directory = False
man_pages = [
    ("pages/simbiota", "simbiota", "Simbiota Client Daemon", "Ukatemi Technologies Zrt.", 8),
    ("pages/simbiota-config", "simbiota_config", "Simbiota Client configuration", "Ukatemi "
                                                                                                 "Technologies Zrt.",
     5),
]