<?php
namespace Drupal\mymodule\Controller;

use Drupal\Core\Controller\ControllerBase;
use Drupal\Core\Form\FormStateInterface;
use Drupal\Core\Link;
use Drupal\Core\Url;

class PageController extends ControllerBase
{
    public function page()
    {
        Url::fromRoute('mymodule.page');
        Link::createFromRoute('mymodule.page', 'mymodule.page');
        return $this->redirect('mymodule.page');
    }

    public function title()
    {
    }

    public function access()
    {
    }

    public function submit(array &$form, FormStateInterface $form_state)
    {
        $form_state->setRedirect('mymodule.page');
        Url::fromRoute('mymodule.nowhere');
    }
}
