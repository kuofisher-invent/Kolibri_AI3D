require 'sketchup'
require 'extensions'

module Kolibri
  module SkpBridge
    EXTENSION = SketchupExtension.new(
      'Kolibri SKP Bridge',
      File.join('kolibri_skp_bridge', 'main')
    )
    EXTENSION.version     = '0.1.0'
    EXTENSION.creator     = 'Kolibri Design'
    EXTENSION.copyright   = '2026'
    EXTENSION.description = 'Exports SketchUp scene graph data for Kolibri SKP import.'

    Sketchup.register_extension(EXTENSION, true)
  end
end
