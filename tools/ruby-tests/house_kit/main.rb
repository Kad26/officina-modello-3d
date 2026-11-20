module HouseKit
  LH = LanguageHandler.new('house_kit.strings')

  def self.house
    model = Sketchup.active_model
    model.start_operation('House', true)
    g = model.active_entities.add_group
    g.name = 'House'
    e = g.entities
    w, d, h, ridge = 240.0, 360.0, 96.0, 156.0
    base = e.add_face([0, 0, 0], [w, 0, 0], [w, d, 0], [0, d, 0])
    base.reverse! if base.normal.z < 0
    base.pushpull(h)
    roof = model.materials.add('Roof')
    roof.color = [90, 90, 100]
    left = e.add_face([0, 0, h], [w / 2, 0, ridge], [w / 2, d, ridge], [0, d, h])
    right = e.add_face([w, 0, h], [w, d, h], [w / 2, d, ridge], [w / 2, 0, ridge])
    e.add_face([0, 0, h], [w, 0, h], [w / 2, 0, ridge])
    e.add_face([0, d, h], [w / 2, d, ridge], [w, d, h])
    left.material = roof
    right.material = roof
    model.commit_operation
  end

  unless file_loaded?(__FILE__)
    cmd = UI::Command.new(LH['Build a house']) { HouseKit.house }
    cmd.tooltip = 'Build a house'
    UI.menu('Plugins').add_item(cmd)
    tb = UI::Toolbar.new('House Kit')
    tb.add_item(cmd)
    tb.show
    file_loaded(__FILE__)
  end
end
