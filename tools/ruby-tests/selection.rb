# Selection: paint what's selected red, and say how many.
UI.menu('Plugins').add_item('Paint selection red') do
  sel = Sketchup.active_model.selection
  if sel.empty?
    UI.messagebox('Select something first')
  else
    sel.each { |e| e.material = 'red' }
    puts "Painted #{sel.length} red"
    UI.messagebox("Painted #{sel.length}")
  end
end
