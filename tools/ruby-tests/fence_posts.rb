# Fence posts: a component definition, placed ten times along red.
module FenceTest
  def self.posts
    model = Sketchup.active_model
    defn = model.definitions.add('Post')
    f = defn.entities.add_face([0, 0, 0], [4, 0, 0], [4, 4, 0], [0, 4, 0])
    f.reverse! if f.normal.z < 0
    f.pushpull(36)
    10.times { |i| model.active_entities.add_instance(defn, Geom::Transformation.new([i * 24, 120, 0])) }
  end
end

UI.menu('Plugins').add_item('Fence posts') { FenceTest.posts }
