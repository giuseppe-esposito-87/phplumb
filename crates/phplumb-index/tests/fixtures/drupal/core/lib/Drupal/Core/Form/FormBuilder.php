<?php
namespace Drupal\Core\Form;

use Drupal\Core\Extension\ModuleHandlerInterface;
use Drupal\Core\Theme\ThemeManagerInterface;

class FormBuilder
{
    public function __construct(
        protected ModuleHandlerInterface $moduleHandler,
        protected ThemeManagerInterface $themeManager,
    ) {
    }

    public function prepareForm($form_id, &$form, &$form_state)
    {
        $hooks = ['form'];
        $hooks[] = 'form_' . $form_id;
        $this->moduleHandler->/*@target:form_id*/alter($hooks, $form, $form_state, $form_id);
        $this->themeManager->/*@target:theme_form*/alter($hooks, $form, $form_state, $form_id);
    }
}
