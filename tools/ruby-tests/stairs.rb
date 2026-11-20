# Stairs: an input box with a list, nested groups, reverse! then pushpull,
# a material by color, and moving the whole flight.
require 'sketchup.rb'

module StairsTest
  def self.build
    prompts = ['Steps', 'Rise', 'Run', 'Width', 'Material']
    defaults = [8, 7.0, 11.0, 36.0, 'Wood']
    list = ['', '', '', '', 'Wood|Concrete|Steel']
    r = UI.inputbox(prompts, defaults, list, 'Stairs')
    return unless r
    n, rise, run, width, name = r
    model = Sketchup.active_model
    model.start_operation('Stairs', true)
    mat = model.materials.add(name)
    mat.color = name == 'Wood' ? Sketchup::Color.new(160, 110, 60) : 'gray'
    flight = model.active_entities.add_group
    flight.name = 'Stairs'
    n.times do |i|
      step = flight.entities.add_group
      step.name = "Step #{i + 1}"
      z = i * rise
      f = step.entities.add_face([i * run, 0, z], [(i + 1) * run, 0, z], [(i + 1) * run, width, z], [i * run, width, z])
      f.reverse! if f.normal.z < 0
      f.pushpull(rise)
      step.material = mat
    end
    flight.transform!(Geom::Transformation.translation([0, -60, 0]))
    model.commit_operation
  end
end

UI.menu('Plugins').add_item('Stairs…') { StairsTest.build } unless file_loaded?(__FILE__)
file_loaded(__FILE__)
