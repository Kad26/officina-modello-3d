# House Kit: packaged the way Extension Warehouse extensions are.
require 'sketchup.rb'
require 'extensions.rb'

module HouseKit
  unless file_loaded?(__FILE__)
    ext = SketchupExtension.new('House Kit', 'house_kit/main')
    ext.description = 'A little house with a gable roof'
    ext.version = '1.0.0'
    ext.creator = 'SketchCraft tests'
    Sketchup.register_extension(ext, true)
    file_loaded(__FILE__)
  end
end
