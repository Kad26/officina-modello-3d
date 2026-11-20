# Uses a web dialog, which SketchCraft doesn't have: should say so clearly.
UI.menu('Plugins').add_item('Open a dialog') do
  dlg = UI::HtmlDialog.new(dialog_title: 'Hello')
  dlg.show
end
