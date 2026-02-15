import * as React from 'react'
import { Slot } from '@radix-ui/react-slot'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const buttonVariants = cva(
  'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-[color,background-color,border-color,box-shadow,transform] duration-150 active:duration-75 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-foreground focus-visible:ring-offset-4 focus-visible:ring-offset-background disabled:opacity-40 disabled:cursor-not-allowed disabled:saturate-0 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0',
  {
    variants: {
      variant: {
        default:
          'bg-primary text-primary-foreground shadow-md hover:bg-primary/85 hover:shadow-lg hover:-translate-y-0.5 active:scale-[0.95] active:shadow-none active:translate-y-0',
        destructive:
          'bg-destructive text-destructive-foreground shadow-md hover:bg-destructive/85 hover:shadow-lg hover:-translate-y-0.5 active:scale-[0.95] active:shadow-none active:translate-y-0',
        outline:
          'border border-input bg-background shadow-sm hover:bg-accent hover:text-accent-foreground hover:border-accent-foreground/20 hover:shadow-md hover:-translate-y-0.5 active:scale-[0.95] active:shadow-none active:translate-y-0 active:bg-accent/80',
        secondary:
          'bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/70 hover:shadow-md hover:-translate-y-0.5 active:scale-[0.95] active:shadow-none active:translate-y-0',
        link: 'text-primary underline-offset-4 hover:underline hover:text-primary/80 active:text-primary/60',
        glow: 'bg-primary text-primary-foreground shadow-md glow-sm hover:shadow-lg hover:glow hover:-translate-y-0.5 active:scale-[0.95] active:shadow-none active:translate-y-0',
      },
      size: {
        default: 'h-10 px-4 py-2',
        sm: 'h-9 rounded-md px-3 text-xs',
        lg: 'h-11 rounded-md px-8',
        icon: 'h-10 w-10',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  }
)

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : 'button'
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    )
  }
)
Button.displayName = 'Button'

export { Button, buttonVariants }
