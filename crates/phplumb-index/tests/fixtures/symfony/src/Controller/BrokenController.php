<?php

namespace App\Controller;

use Symfony\Bridge\Twig\Attribute\Template;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\DependencyInjection\Attribute\Autowire;

class BrokenController extends AbstractController
{
    public function __construct(
        #[Autowire(param: 'app.missing')] private string $missing,
        #[Autowire('%app.name% and %app.gone%')] private string $mixed,
    ) {
    }

    public function broken(): void
    {
        $this->redirectToRoute('home');
        $this->redirectToRoute('missing_route');
        $this->redirectToRoute('_wdt');
        $this->render('base.html.twig');
        $this->render('nowhere.html.twig');
        $this->render('@Unknown/x.html.twig');
        $this->getParameter('app.name');
        $this->getParameter('app.nowhere');
        $this->getParameter('router.request_context.host');
    }

    #[Template('gone.html.twig')]
    public function shown(): array
    {
        return [];
    }
}
